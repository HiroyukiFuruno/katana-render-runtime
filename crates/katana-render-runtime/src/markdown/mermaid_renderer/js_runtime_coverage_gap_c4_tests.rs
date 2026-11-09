use super::super::{GeometryContract, tests};

#[test]
fn c4_validator_reports_wrong_root_and_duplicate_inventory() {
    assert_eq!(
        GeometryContract::dynamic("<root/>", false).err(),
        Some("SVG root required".to_owned())
    );
    let input = tests::C4.replace("</svg>", r#"<g class="c4-container" id="id-extra"/></svg>"#);
    assert_eq!(
        GeometryContract::dynamic(&input, false).err(),
        Some("C4 node inventory changed".to_owned())
    );
}

#[test]
fn c4_validator_reports_technology_assigned_to_wrong_node() {
    let input = tests::C4.replace("javascript and react", "unrelated technology");
    assert_eq!(
        GeometryContract::dynamic(&input, false).err(),
        Some("C4 technology belongs to wrong node".to_owned())
    );
}

#[test]
fn c4_validator_reports_missing_rect_and_relation() {
    let rectless = tests::C4.replace(
        r#"<rect class="label-container" width="80" height="80"/>"#,
        "",
    );
    assert_eq!(
        GeometryContract::dynamic(&rectless, false).err(),
        Some("semantic element must be unique".to_owned())
    );
    let edgeless = tests::C4.replace(
        r#"<line x1="80" y1="40" x2="200" y2="40" marker-end="url(#arrow)"/>"#,
        "",
    );
    assert_eq!(
        GeometryContract::dynamic(&edgeless, false).err(),
        Some("semantic element must be unique".to_owned())
    );
}

#[test]
fn c4_validator_reports_nonfinite_and_collapsed_rectangles() {
    let nonfinite = tests::C4.replace(r#"width="80""#, r#"width="NaN""#);
    assert_eq!(
        GeometryContract::dynamic(&nonfinite, false).err(),
        Some("non-finite geometry".to_owned())
    );
    let collapsed = tests::C4.replace(
        r#"<rect class="label-container" width="80" height="80"/>"#,
        r#"<rect class="label-container" width="80" height="80" transform="scale(0)"/>"#,
    );
    assert_eq!(
        GeometryContract::dynamic(&collapsed, false).err(),
        Some("transformed rectangle collapsed".to_owned())
    );
}

#[test]
fn c4_validator_reports_reversed_relation_and_detached_label() {
    let reversed = tests::C4.replace(
        r#"x1="80" y1="40" x2="200" y2="40""#,
        r#"x1="200" y1="40" x2="80" y2="40""#,
    );
    assert_eq!(
        GeometryContract::dynamic(&reversed, false).err(),
        Some("C4 relation endpoints detached or reversed".to_owned())
    );
    let detached = tests::C4.replace(r#"x="140""#, r#"x="290""#);
    assert_eq!(
        GeometryContract::dynamic(&detached, false).err(),
        Some("C4 relation label detached from edge corridor".to_owned())
    );
}

#[test]
fn c4_validator_rejects_missing_api_identity_and_type() -> Result<(), String> {
    GeometryContract::dynamic(tests::C4, false)?;
    for (before, after) in [
        (r#"id="id-api""#, r#"id="id-other""#),
        (
            r#"class="c4-type"><text x="40" y="35">Java and Spring Boot"#,
            r#"class="other"><text x="40" y="35">Java and Spring Boot"#,
        ),
    ] {
        assert_eq!(tests::C4.matches(before).count(), 1);
        let changed = tests::C4.replacen(before, after, 1);
        assert_eq!(
            GeometryContract::dynamic(&changed, false).err().as_deref(),
            Some("semantic element must be unique")
        );
    }
    Ok(())
}

#[test]
fn c4_validator_rejects_relation_without_target_coordinate() -> Result<(), String> {
    GeometryContract::dynamic(tests::C4, false)?;
    let before = r#"x2="200" y2="40""#;
    assert_eq!(tests::C4.matches(before).count(), 1);
    let changed = tests::C4.replacen(before, r#"y2="40""#, 1);
    assert_eq!(
        GeometryContract::dynamic(&changed, false).err().as_deref(),
        Some("missing x2")
    );
    Ok(())
}
