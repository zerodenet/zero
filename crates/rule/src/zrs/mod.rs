mod encode;
mod layout;
mod runtime;
mod verify;

use thiserror::Error;

pub use encode::encode;
pub use layout::{
    MAJOR_VERSION, MAX_ENTRIES_PER_SECTION, MAX_FILE_SIZE, MAX_SECTION_COUNT, MAX_SECTION_SIZE,
    MINOR_VERSION,
};
pub use runtime::{MappedRuleSet, PrewarmPolicy, PrewarmReport, VerifiedRuleSet};
pub use verify::{verify, RuleSectionBytes, RuleSetMetadata, RuleTypeCounts, VerifyMode};

#[derive(Debug, Error)]
pub enum ZrsError {
    #[error("too many entries for ZRS 0.1")]
    TooManyEntries,
    #[error("ZRS file size overflow")]
    FileTooLarge,
    #[error("failed to build FST section: {0}")]
    FstBuild(#[source] fst::Error),
    #[error("ZRS file is truncated")]
    Truncated,
    #[error("invalid ZRS magic")]
    InvalidMagic,
    #[error("unsupported ZRS version {major}.{minor}")]
    UnsupportedVersion { major: u16, minor: u16 },
    #[error("invalid ZRS header: {0}")]
    InvalidHeader(&'static str),
    #[error("invalid ZRS section {kind}: {reason}")]
    InvalidSection { kind: u16, reason: &'static str },
    #[error("missing required ZRS section {0}")]
    MissingSection(u16),
    #[error("duplicate ZRS section {0}")]
    DuplicateSection(u16),
    #[error("ZRS body checksum mismatch")]
    ChecksumMismatch,
    #[error("invalid UTF-8 in ZRS metadata or string table")]
    InvalidUtf8,
    #[error("invalid FST section: {0}")]
    InvalidFst(#[source] fst::Error),
    #[error("failed to open or map ZRS file: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZRS {resource} is {actual}; maximum is {maximum}")]
    ResourceLimit {
        resource: &'static str,
        actual: u64,
        maximum: u64,
    },
}
