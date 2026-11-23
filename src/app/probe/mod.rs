pub mod asn;
pub mod dns;
pub mod http;
pub mod process;
pub mod rdap;
pub mod scope;
pub mod subfinder;
pub mod types;

// Re-exports for backwards compatibility
pub use asn::probe_asn_batch;
pub use dns::{probe_dns, probe_dns_batch, probe_dns_batch_no_save};
pub use http::{http_check, probe_http_batch, probe_http_batch_no_save};
pub use rdap::{run_rdap_probe, run_rdap_probe_no_save};
pub use subfinder::{detect_junk_candidates, probe_subdomains, probe_subdomains_batch_no_save};
#[allow(unused_imports)] // Used in tests
pub use types::{
    AsnObservation, Confidence, DetectionMethod, DnsObservation, HostingObservation, HttpErrorKind,
    HttpObservation, RedirectHop,
};

use std::sync::atomic::{AtomicBool, Ordering};

static QUIET: AtomicBool = AtomicBool::new(false);

/// Suppress probe stdout progress messages (used by TUI mode).
pub fn set_quiet(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

/// Returns `true` when probe stdout progress messages are suppressed.
pub(crate) fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}
