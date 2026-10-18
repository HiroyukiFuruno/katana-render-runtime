use super::super::{GeometryContract, tests};

#[test]
fn architecture_validator_reports_collapsed_real_source_edge() -> Result<(), String> {
    tests::architecture_pair(tests::ARCH)?;
    let input = tests::ARCH.replace("M80 40L200 40", "M80 40L80 40");
    assert_eq!(
        GeometryContract::architecture(
            &input,
            &[("a", "A"), ("b", "B")],
            &[("a", "b", 'R', 'L')],
            ("app", "Group"),
        )
        .err(),
        Some("edge must not collapse".to_owned())
    );
    Ok(())
}
