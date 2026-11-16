#![cfg(test)]

use super::{mode, require_ack, Mode};
use std::{ffi::OsStr, io::Cursor};

#[test]
fn modes_preserve_standard_count_and_extend_diagnostic() {
    assert_eq!(mode(Some(OsStr::new("--retention"))), Some(Mode::Standard));
    assert_eq!(
        mode(Some(OsStr::new("--retention-diagnostic"))),
        Some(Mode::Diagnostic)
    );
    assert_eq!(mode(Some(OsStr::new("--worker"))), None);
    assert_eq!(Mode::Standard.total_cycles(), 30);
    assert_eq!(Mode::Diagnostic.total_cycles(), 105);
}

#[test]
fn acknowledgement_requires_the_exact_cycle_and_newline() {
    for (input, accepted) in [
        ("ACK 4\n", true),
        ("ACK 3\n", false),
        ("ACK 4", false),
        ("", false),
    ] {
        assert_eq!(require_ack(&mut Cursor::new(input), 4).is_ok(), accepted);
    }
}
