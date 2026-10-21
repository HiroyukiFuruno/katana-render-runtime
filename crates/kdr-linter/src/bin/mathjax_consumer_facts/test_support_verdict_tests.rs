pub(super) fn assert_rejected<T>(
    result: Result<T, String>,
    expected: &str,
    accepted_message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Err(error) = result else {
        return Err(accepted_message.into());
    };
    assert!(
        error.contains(expected),
        "expected rejection {expected}, actual: {error}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::assert_rejected;

    #[test]
    fn accepts_expected_rejection() {
        assert!(
            assert_rejected::<()>(Err("route-shape".to_string()), "route-shape", "accepted")
                .is_ok()
        );
    }

    #[test]
    fn rejects_erroneous_acceptance() {
        let result = assert_rejected(Ok(()), "route-shape", "mutated route was accepted");
        assert!(result.is_err_and(|error| error.to_string() == "mutated route was accepted"));
    }

    #[test]
    #[should_panic(expected = "expected rejection route-shape, actual: other-shape")]
    fn rejects_wrong_diagnostic() {
        let _ = assert_rejected::<()>(Err("other-shape".to_string()), "route-shape", "accepted");
    }
}
