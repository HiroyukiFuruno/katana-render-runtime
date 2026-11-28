#![cfg(test)]

use super::{
    test_support::{
        case_with_report, fixture_source_hash, operation_report, STATIC_OPERATION_KIND,
    },
    validate_sample_input,
};
use crate::options::OperationPlan;

#[test]
fn validates_requested_selector_against_catalog_target_evidence(
) -> Result<(), Box<dyn std::error::Error>> {
    let source_hash = fixture_source_hash()?;
    for (selector, expected_error) in [
        ("#state", None),
        ("#other", Some("catalog operation target evidence differs")),
    ] {
        let mut case = case_with_report(
            operation_report(&source_hash, STATIC_OPERATION_KIND),
            &source_hash,
            STATIC_OPERATION_KIND,
        )?;
        if let OperationPlan::Catalog(plan) = &mut case.plan {
            plan.target_selector = Some(selector.to_owned());
        }

        match (
            expected_error,
            validate_sample_input(&case.plan, &case.source_hash, case.viewport),
        ) {
            (None, Ok(false)) => {}
            (Some(expected), Err(error)) => assert_eq!(error.to_string(), expected, "{selector}"),
            (None, Ok(true)) => return Err("static operation unexpectedly settled".into()),
            (Some(_), Ok(_)) => return Err("mismatched selector was accepted".into()),
            (None, Err(error)) => return Err(error),
        }
    }
    Ok(())
}
