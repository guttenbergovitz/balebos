//! Observation outcome semantics.
//!
//! An outcome describes what an inspection attempt produced. It is evidence
//! vocabulary (ADR 0005), not assessment: an outcome never changes what
//! exists, only what was observed, skipped or left unknown.

use serde::{Deserialize, Serialize};

/// The distinguished result of an inspection attempt.
///
/// Every variant is explicit so that "unknown" is never silently treated as
/// "healthy" and incomplete work is never presented as complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationOutcome {
    /// The inspection completed and produced a trustworthy result.
    Success,
    /// The inspection was attempted but failed; carries the recorded error.
    Failed {
        /// Normalised error description from the adapter.
        error: String,
    },
    /// The inspection was deliberately not attempted for this target.
    Skipped,
    /// The inspection method cannot handle this target at all.
    Unsupported,
    /// The inspection started but did not finish; the result is partial.
    Incomplete,
    /// A result exists but cannot be trusted as complete or correct.
    Unverified,
    /// The method genuinely supports absence: the thing is confirmed absent.
    ConfirmedAbsent,
}

impl ObservationOutcome {
    /// True when a trustworthy, complete result was produced.
    pub fn is_success(&self) -> bool {
        matches!(self, ObservationOutcome::Success)
    }

    /// True when the target was inspected in any form (success, failure,
    /// partial or untrusted results all count as observed attempts).
    pub fn was_attempted(&self) -> bool {
        !matches!(
            self,
            ObservationOutcome::Skipped | ObservationOutcome::Unsupported
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_serialises_to_stable_snake_case_forms() {
        let cases = [
            (ObservationOutcome::Success, r#""success""#),
            (
                ObservationOutcome::Failed {
                    error: "timeout".to_string(),
                },
                r#"{"failed":{"error":"timeout"}}"#,
            ),
            (ObservationOutcome::Skipped, r#""skipped""#),
            (ObservationOutcome::Unsupported, r#""unsupported""#),
            (ObservationOutcome::Incomplete, r#""incomplete""#),
            (ObservationOutcome::Unverified, r#""unverified""#),
            (ObservationOutcome::ConfirmedAbsent, r#""confirmed_absent""#),
        ];

        for (outcome, expected) in cases {
            let json = serde_json::to_string(&outcome).unwrap();
            assert_eq!(json, expected, "stable serialised form changed");
            let back: ObservationOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(back, outcome);
        }
    }

    #[test]
    fn success_and_attempted_predicates() {
        assert!(ObservationOutcome::Success.is_success());
        assert!(ObservationOutcome::Success.was_attempted());
        assert!(ObservationOutcome::Incomplete.was_attempted());
        assert!(ObservationOutcome::Failed {
            error: "x".to_string()
        }
        .was_attempted());
        assert!(!ObservationOutcome::Skipped.was_attempted());
        assert!(!ObservationOutcome::Unsupported.was_attempted());
        assert!(!ObservationOutcome::Unverified.is_success());
    }
}
