#![cfg(test)]

use super::require_ack;
use std::io::Cursor;

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
