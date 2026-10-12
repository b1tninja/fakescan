//! XSD well-known Scanner/Job state and reason tokens (catalog ratchet).
//!
//! Sources: `docs/reference/schema/WellKnownValues.xsd`, `Scanner.xsd`
//! (`ConditionNameBaseType`, `ConditionSeverityBaseType`). Coverage map:
//! `docs/coverage.md` § Scanner / job state matrix.

/// `ScannerStateWKVType`
pub const SCANNER_STATES: &[&str] = &["Idle", "Processing", "Stopped"];

/// `ScannerStateReasonWKVType`
pub const SCANNER_STATE_REASONS: &[&str] = &[
    "AttentionRequired",
    "Calibrating",
    "CoverOpen",
    "InterlockOpen",
    "InternalStorageFull",
    "LampError",
    "LampWarming",
    "MediaJam",
    "MultipleFeedError",
    "None",
    "Paused",
];

/// `ConditionNameBaseType` (Scanner.xsd) — ActiveConditions / ConditionHistory Name.
pub const CONDITION_NAMES: &[&str] = &[
    "Calibrating",
    "CoverOpen",
    "InputTrayEmpty",
    "InterlockOpen",
    "InternalStorageFull",
    "MediaJam",
    "LampError",
    "LampWarning",
    "MultipleFeedError",
];

/// `JobStateWKVType`
pub const JOB_STATES: &[&str] = &[
    "Aborted",
    "Canceled",
    "Completed",
    "Creating",
    "Pending",
    "Pending-Held",
    "Processing",
    "Started",
    "Terminating",
];

/// `JobStateReasonWKVType`
pub const JOB_STATE_REASONS: &[&str] = &[
    "InvalidScanTicket",
    "CompressionError",
    "DocumentFormatError",
    "JobCanceledAtDevice",
    "JobCompletedSuccessfully",
    "JobCompletedWithErrors",
    "JobCompletedWithWarnings",
    "JobScanning",
    "JobScanningAndTransfering",
    "JobTimedOut",
    "JobTransfering",
    "None",
    "ScannerStopped",
];

/// `ConditionSeverityBaseType` (not `Error` — that was invalid).
pub const CONDITION_SEVERITIES: &[&str] = &["Informational", "Warning", "Critical"];

#[cfg(test)]
mod tests {
    use super::*;

    fn contains(hay: &[&str], needle: &str) -> bool {
        hay.contains(&needle)
    }

    #[test]
    fn known_reasons_are_documented() {
        assert!(!SCANNER_STATES.is_empty());
        assert!(!SCANNER_STATE_REASONS.is_empty());
        assert!(!CONDITION_NAMES.is_empty());
        assert!(!JOB_STATES.is_empty());
        assert!(!JOB_STATE_REASONS.is_empty());
        assert!(!CONDITION_SEVERITIES.is_empty());

        assert!(contains(SCANNER_STATE_REASONS, "MediaJam"));
        assert!(contains(SCANNER_STATE_REASONS, "AttentionRequired"));
        assert!(contains(SCANNER_STATE_REASONS, "CoverOpen"));
        assert!(contains(SCANNER_STATE_REASONS, "LampError"));
        assert!(contains(SCANNER_STATE_REASONS, "MultipleFeedError"));
        assert!(contains(SCANNER_STATE_REASONS, "None"));

        assert!(contains(CONDITION_NAMES, "MediaJam"));
        assert!(contains(CONDITION_NAMES, "InputTrayEmpty"));
        assert!(contains(CONDITION_NAMES, "LampWarning"));

        assert!(contains(JOB_STATE_REASONS, "JobTimedOut"));
        assert!(contains(JOB_STATE_REASONS, "JobCompletedSuccessfully"));
        assert!(contains(JOB_STATE_REASONS, "JobCompletedWithWarnings"));
        assert!(contains(JOB_STATE_REASONS, "JobCompletedWithErrors"));
        assert!(contains(JOB_STATE_REASONS, "JobCanceledAtDevice"));
        assert!(contains(JOB_STATE_REASONS, "JobScanning"));
        assert!(contains(JOB_STATE_REASONS, "JobScanningAndTransfering"));
        assert!(contains(JOB_STATE_REASONS, "JobTransfering"));
        assert!(contains(JOB_STATE_REASONS, "ScannerStopped"));
        assert!(contains(JOB_STATE_REASONS, "CompressionError"));
        assert!(contains(JOB_STATE_REASONS, "DocumentFormatError"));
        assert!(contains(JOB_STATE_REASONS, "None"));

        assert!(contains(JOB_STATES, "Pending"));
        assert!(contains(JOB_STATES, "Processing"));
        assert!(contains(JOB_STATES, "Creating"));

        assert!(contains(CONDITION_SEVERITIES, "Critical"));
        assert!(!contains(CONDITION_SEVERITIES, "Error"));
    }
}
