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

pub(super) fn assert_unique_anchor(source: &str, anchor: &str) -> Result<(), String> {
    let count = source.matches(anchor).count();
    if count != 1 {
        return Err(format!(
            "mutation anchor expected one match, found {count}: {anchor}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{assert_rejected, assert_unique_anchor};

    #[test]
    fn requires_exactly_one_mutation_anchor() {
        assert!(assert_unique_anchor("anchor", "anchor").is_ok());
        assert!(assert_unique_anchor("", "anchor").is_err());
        assert!(assert_unique_anchor("anchor anchor", "anchor").is_err());
    }

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
