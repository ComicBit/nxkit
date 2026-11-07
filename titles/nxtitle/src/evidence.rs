//! Where a fact comes from.

/// The evidence for one fact. A fact without a check says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidence {
    /// A test in this workspace proves it.
    Tested {
        /// The test, as `crate::module::test_name`.
        test: &'static str,
    },
    /// A record outside this workspace states it, with the test or the measurement behind it.
    Recorded {
        /// The record, as `repository path at commit` or a URL.
        source: &'static str,
        /// The day the record was read (`YYYY-MM-DD`).
        read: &'static str,
    },
    /// Believed and not checked.
    Unverified {
        /// Why it is believed, and what would check it.
        note: &'static str,
    },
}

impl Evidence {
    /// Whether a test or a record backs the fact.
    pub fn is_checked(&self) -> bool {
        !matches!(self, Evidence::Unverified { .. })
    }
}

/// A value with its evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact<T> {
    /// The value.
    pub value: T,
    /// Where it comes from.
    pub evidence: Evidence,
}

impl<T> Fact<T> {
    /// A fact.
    pub const fn new(value: T, evidence: Evidence) -> Fact<T> {
        Fact { value, evidence }
    }
}
