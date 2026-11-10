use super::super::super::reference_score_tests::{
    ARCHITECTURE_SERVICE_ICONS_SOURCE, render_mermaid_svg,
};
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

#[test]
fn architecture_validator_requires_the_service_inventory_and_unique_group() -> Result<(), String> {
    tests::architecture_pair(tests::ARCH)?;
    for (before, after, expected) in [
        (
            r#"id="id-service-b" class="architecture-service""#,
            r#"id="id-service-b" class="other""#,
            "service inventory changed",
        ),
        (
            r#"id="id-group-app""#,
            r#"id="id-group-other""#,
            "semantic element must be unique",
        ),
        (
            "</svg>",
            r#"<rect id="extra-group-app" width="20" height="20"/></svg>"#,
            "semantic element must be unique",
        ),
    ] {
        assert_eq!(tests::ARCH.matches(before).count(), 1);
        let changed = tests::ARCH.replacen(before, after, 1);
        assert_eq!(
            tests::architecture_pair(&changed).err().as_deref(),
            Some(expected)
        );
    }
    Ok(())
}

#[test]
fn architecture_icons_reject_missing_source_icon_shapes() -> Result<(), String> {
    let svg = render_mermaid_svg(ARCHITECTURE_SERVICE_ICONS_SOURCE)?;
    GeometryContract::icons(&svg)?;
    for (before, after, expected) in [
        (
            r#"rx="20" ry="7.14""#,
            r#"rx="19" ry="7.14""#,
            "database source icon lost",
        ),
        ("l-4.83,13.22", "l-4.82,13.22", "disk source icon lost"),
    ] {
        assert!(svg.contains(before));
        assert_eq!(
            GeometryContract::icons(&svg.replace(before, after))
                .err()
                .as_deref(),
            Some(expected)
        );
    }
    Ok(())
}
