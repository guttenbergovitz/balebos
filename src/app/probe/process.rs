//! Bounded subprocess execution for probe adapters (security-audit
//! finding: uniform timeouts, kill and reap; brief §8 explicit bounds).
//!
//! `run_bounded` executes a command with an outer envelope: wall-clock
//! timeout, bounded captured output, and — on Unix — process-group
//! termination so no grandchildren outlive a timed-out scanner. Killed
//! children are always reaped (waited), never left as zombies.
//!
//! This is the outer envelope only; per-request timeouts inside scanners
//! (e.g. httpx `-timeout`) remain the tool's own concern.

use std::io::Read;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::error::{BalebosError, Result};

const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Operator cancellation (SIGINT): set by the signal handler in main,
/// checked by every bounded execution watchdog.
static CANCELLED: AtomicBool = AtomicBool::new(false);

/// Request cancellation of in-flight scanner processes.
pub fn cancel() {
    CANCELLED.store(true, Ordering::SeqCst);
}

/// Clear the cancellation flag (test isolation and fresh CLI runs).
pub fn reset_cancel() {
    CANCELLED.store(false, Ordering::SeqCst);
}

pub fn is_cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

fn process_err(msg: impl Into<String>) -> BalebosError {
    BalebosError::Probe(crate::error::ProbeError::Execution(std::io::Error::other(
        msg.into(),
    )))
}

/// Default ceiling for captured scanner output (64 MiB). Scanners that
/// legitimately exceed this are misconfigured, not busy.
pub const DEFAULT_MAX_OUTPUT: usize = 64 * 1024 * 1024;

/// Execute `cmd` within `timeout`, capturing at most `max_output` bytes
/// per stream. On timeout the process (and its group, on Unix) is killed
/// and reaped before the error returns.
pub fn run_bounded(cmd: &mut Command, timeout: Duration, max_output: usize) -> Result<Output> {
    configure_command(cmd);
    let child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BalebosError::Probe(crate::error::ProbeError::ToolNotFound {
                    tool: program_name(cmd),
                })
            } else {
                BalebosError::Probe(crate::error::ProbeError::Execution(e))
            }
        })?;
    finish_bounded(child, timeout, max_output)
}

/// Attach the timeout envelope to an already-spawned child (streaming
/// adapters pipe stdin/stdout themselves and only need bounded waiting).
pub fn wait_bounded(child: Child, timeout: Duration, max_output: usize) -> Result<Output> {
    finish_bounded(child, timeout, max_output)
}

/// Execute `cmd` with optional `stdin_data`, streaming `stdout` line-by-line
/// to `on_line` while draining `stderr` concurrently and enforcing `timeout`
/// and operator cancellation from the moment the process starts.
pub fn stream_bounded(
    cmd: &mut Command,
    stdin_data: Option<String>,
    timeout: Duration,
    max_output: usize,
    on_line: &dyn Fn(&str),
) -> Result<std::process::ExitStatus> {
    use std::io::{BufRead, BufReader, Write};
    use std::sync::mpsc::{self, RecvTimeoutError};

    configure_command(cmd);
    let mut child = cmd
        .stdin(if stdin_data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BalebosError::Probe(crate::error::ProbeError::ToolNotFound {
                    tool: program_name(cmd),
                })
            } else {
                BalebosError::Probe(crate::error::ProbeError::Execution(e))
            }
        })?;

    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let start = Instant::now();

    std::thread::scope(|scope| {
        if let (Some(mut pipe), Some(data)) = (stdin, stdin_data) {
            scope.spawn(move || {
                let _ = pipe.write_all(data.as_bytes());
            });
        }

        let stderr_handle = scope.spawn(move || {
            stderr.map_or_else(|| (Vec::new(), false), |s| read_capped(s, max_output))
        });

        let (tx, rx) = mpsc::channel::<String>();
        let stdout_handle = scope.spawn(move || -> bool {
            let Some(out) = stdout else {
                return false;
            };
            let mut reader = BufReader::new(out);
            let mut total_bytes = 0usize;
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => return false,
                    Ok(n) => {
                        total_bytes = total_bytes.saturating_add(n);
                        if total_bytes > max_output {
                            return true;
                        }
                        let trimmed = line.trim_end_matches(['\r', '\n']);
                        if tx.send(trimmed.to_string()).is_err() {
                            return false;
                        }
                    }
                    Err(_) => return false,
                }
            }
        });

        let mut aborted_err: Option<BalebosError> = None;
        loop {
            match rx.recv_timeout(POLL_INTERVAL) {
                Ok(line) => {
                    on_line(&line);
                }
                Err(RecvTimeoutError::Timeout) => {
                    if is_cancelled() {
                        terminate_group(&mut child);
                        aborted_err = Some(process_err("cancelled by the operator"));
                        break;
                    }
                    if start.elapsed() > timeout {
                        terminate_group(&mut child);
                        aborted_err = Some(process_err(format!(
                            "scanner exceeded the {:?} timeout and was terminated",
                            timeout
                        )));
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        drop(rx);
        let status = child
            .wait()
            .map_err(|e| BalebosError::Probe(crate::error::ProbeError::Execution(e)))?;

        let stdout_overflow = stdout_handle.join().unwrap_or(false);
        let (_stderr_data, stderr_overflow) = stderr_handle.join().unwrap_or((Vec::new(), false));

        if let Some(err) = aborted_err {
            return Err(err);
        }
        if stdout_overflow || stderr_overflow {
            return Err(process_err(format!(
                "scanner output exceeded the {} MiB bound",
                max_output / (1024 * 1024)
            )));
        }

        Ok(status)
    })
}

fn finish_bounded(mut child: Child, timeout: Duration, max_output: usize) -> Result<Output> {
    // Streams may already have been taken by a streaming adapter; missing
    // streams read as empty.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Dedicated reader threads: a chatty scanner must never deadlock on a
    // full pipe buffer, and a silent one must never block the timeout
    // loop (reads here are allowed to block; the watch loop is not).
    let stdout_thread = std::thread::spawn(move || {
        stdout.map_or_else(|| (Vec::new(), false), |s| read_capped(s, max_output))
    });
    let stderr_thread = std::thread::spawn(move || {
        stderr.map_or_else(|| (Vec::new(), false), |s| read_capped(s, max_output))
    });

    let start = Instant::now();
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {
                if is_cancelled() {
                    terminate_group(&mut child);
                    break Err(process_err("cancelled by the operator"));
                }
                if start.elapsed() > timeout {
                    terminate_group(&mut child);
                    break Err(process_err(format!(
                        "scanner exceeded the {:?} timeout and was terminated",
                        timeout
                    )));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(e) => {
                terminate_group(&mut child);
                break Err(BalebosError::Probe(crate::error::ProbeError::Execution(e)));
            }
        }
    };

    // Reap after kill (terminate_group already waits; for the success path
    // the status came from try_wait, but a final wait is still required to
    // release the handle). Then collect the readers.
    let status = result?;
    // Release the process handle after the successful try_wait path.
    let _ = child.wait();

    let (stdout_data, stdout_overflow) = stdout_thread
        .join()
        .map_err(|_| process_err("stdout reader thread panicked"))?;
    let (stderr_data, stderr_overflow) = stderr_thread
        .join()
        .map_err(|_| process_err("stderr reader thread panicked"))?;

    if stdout_overflow || stderr_overflow {
        return Err(process_err(format!(
            "scanner output exceeded the {} MiB bound",
            max_output / (1024 * 1024)
        )));
    }

    Ok(Output {
        status,
        stdout: stdout_data,
        stderr: stderr_data,
    })
}

/// Read one stream to EOF, storing at most `max` bytes. Reading continues
/// past the cap (discarding) so the child never blocks on a full pipe.
fn read_capped(mut reader: impl Read, max: usize) -> (Vec<u8>, bool) {
    let mut data = Vec::new();
    let mut overflow = false;
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if !overflow {
                    if data.len() + n > max {
                        overflow = true;
                    } else {
                        data.extend_from_slice(&chunk[..n]);
                    }
                }
            }
            Err(_) => break,
        }
    }
    (data, overflow)
}

/// Put the child in its own process group (Unix) so a timeout can kill
/// the whole group — scanners that fork helpers cannot leak orphans.
fn configure_command(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                nix::unistd::setpgid(nix::unistd::Pid::from_raw(0), nix::unistd::Pid::from_raw(0))
                    .map_err(std::io::Error::from)
            });
        }
    }
}

/// Kill the child and, on Unix, its process group; always reap.
fn terminate_group(child: &mut Child) {
    #[cfg(unix)]
    {
        let pgid = nix::unistd::Pid::from_raw(child.id() as i32);
        let _ = nix::sys::signal::killpg(pgid, nix::sys::signal::Signal::SIGKILL);
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn program_name(cmd: &Command) -> String {
    cmd.get_program().to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cancellation flag is process-global; time-sensitive tests must
    /// not run concurrently with cancellation tests.
    static FLAG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn sh(args: &str) -> Command {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(args);
        cmd
    }

    #[test]
    fn quick_command_succeeds_with_output() {
        let _guard = FLAG_LOCK.lock().unwrap();
        let output = run_bounded(&mut sh("echo hello"), Duration::from_secs(5), 1024).unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
    }

    #[test]
    fn timed_out_process_is_killed_and_reaped() {
        let _guard = FLAG_LOCK.lock().unwrap();
        reset_cancel();
        let start = Instant::now();
        let result = run_bounded(&mut sh("sleep 30"), Duration::from_millis(100), 1024);
        let elapsed = start.elapsed();

        assert!(result.is_err());
        assert!(
            elapsed < Duration::from_secs(5),
            "timeout took too long: {elapsed:?}"
        );
        let err = result.unwrap_err().to_string();
        assert!(err.contains("timeout"), "unexpected error: {err}");
    }

    #[test]
    fn oversized_output_is_rejected() {
        let result = run_bounded(
            &mut sh("yes | head -c 100000"),
            Duration::from_secs(10),
            1024,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeded"));
    }

    #[test]
    fn wait_bounded_applies_to_spawned_children() {
        let _guard = FLAG_LOCK.lock().unwrap();
        reset_cancel();
        let mut cmd = sh("sleep 30");
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_command(&mut cmd);
        let child = cmd.spawn().unwrap();
        let start = Instant::now();
        let result = wait_bounded(child, Duration::from_millis(100), 1024);
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn missing_tool_reports_tool_not_found() {
        let mut cmd = Command::new("definitely-not-a-real-binary-xyz");
        let result = run_bounded(&mut cmd, Duration::from_secs(1), 1024);
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found in PATH"), "unexpected: {err}");
    }

    #[test]
    fn operator_cancellation_terminates_and_reaps() {
        let _guard = FLAG_LOCK.lock().unwrap();
        reset_cancel();
        let start = Instant::now();
        let mut cmd = sh("sleep 30");
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_command(&mut cmd);
        let child = cmd.spawn().unwrap();

        // Simulate the SIGINT handler firing while the watchdog runs.
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            cancel();
        });

        let result = wait_bounded(child, Duration::from_secs(30), 1024);
        reset_cancel();

        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("cancelled"),
            "unexpected error"
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "cancellation took too long"
        );
    }

    #[test]
    fn stream_bounded_streams_lines_drains_stderr_and_enforces_timeout() {
        let mut lines = Vec::new();
        let lines_cell = std::sync::Mutex::new(&mut lines);
        // Emit stdout lines while flooding stderr (>64KiB pipe buffer) concurrently.
        let mut cmd = sh("printf 'one\\ntwo\\n'; head -c 131072 /dev/zero | tr '\\0' 'x' >&2");
        let status = stream_bounded(
            &mut cmd,
            None,
            Duration::from_secs(5),
            1024 * 1024,
            &|line| {
                if let Ok(mut g) = lines_cell.lock() {
                    g.push(line.to_string());
                }
            },
        )
        .unwrap();
        assert!(status.success());
        assert_eq!(lines, vec!["one", "two"]);

        // Silent hung process must be terminated on timeout without blocking on stdout.
        let start = Instant::now();
        let mut hung = sh("sleep 30");
        let err = stream_bounded(&mut hung, None, Duration::from_millis(200), 1024, &|_| {});
        assert!(err.is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
