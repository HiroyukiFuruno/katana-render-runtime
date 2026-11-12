use super::super::super::reference_score_tests::{C4_DEPLOYMENT_SOURCE, render_mermaid_svg};
use super::super::{GeometryContract, tests, text};
use resvg::usvg::roxmltree::Document;

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
fn c4_validator_rejects_names_swapped_between_nodes() -> Result<(), String> {
    GeometryContract::dynamic(tests::C4, false)?;
    let swapped = tests::C4
        .replace(
            "<g class=\"c4-name\"><text x=\"40\" y=\"20\">Single Page Application",
            "<g class=\"c4-name\"><text x=\"40\" y=\"20\">TEMP_NODE_NAME",
        )
        .replace(
            "<g class=\"c4-name\"><text x=\"40\" y=\"20\">API Application",
            "<g class=\"c4-name\"><text x=\"40\" y=\"20\">Single Page Application",
        )
        .replace("TEMP_NODE_NAME", "API Application");
    assert_eq!(
        GeometryContract::dynamic(&swapped, false).err().as_deref(),
        Some("C4 label belongs to wrong node")
    );
    Ok(())
}

#[test]
fn c4_validator_rejects_duplicate_relationship_labels() -> Result<(), String> {
    GeometryContract::dynamic(tests::C4, false)?;
    let duplicated = tests::C4.replace("</svg>", "<text x=\"140\" y=\"70\">Uses</text></svg>");
    assert_eq!(
        GeometryContract::dynamic(&duplicated, false)
            .err()
            .as_deref(),
        Some("semantic element must be unique")
    );
    Ok(())
}

#[test]
fn c4_validator_requires_both_relationship_labels() -> Result<(), String> {
    GeometryContract::dynamic(tests::C4, false)?;
    let missing_protocol = tests::C4.replace("[JSON/HTTPS]", "[HTTP]");
    assert_eq!(
        GeometryContract::dynamic(&missing_protocol, false)
            .err()
            .as_deref(),
        Some("semantic element must be unique")
    );
    Ok(())
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

#[test]
fn c4_deployment_rejects_a_real_label_displaced_from_its_parent() -> Result<(), String> {
    let svg = validated_deployment_svg()?;
    let document = Document::parse(&svg).map_err(|error| error.to_string())?;
    let label = document
        .descendants()
        .find(|node| node.has_tag_name("text") && text(*node) == "[Apple iOS or Android]")
        .ok_or("deployment technology label missing")?;
    let position = label
        .attribute_node("x")
        .ok_or("label x position missing")?;
    let range = position.range_value();
    let mut displaced = svg.clone();
    displaced.replace_range(range, "1000000");
    assert_eq!(
        GeometryContract::deployment(&displaced).err().as_deref(),
        Some("deployment labels detached from parent")
    );
    Ok(())
}

#[test]
fn c4_deployment_rejects_missing_parent_labels() -> Result<(), String> {
    let svg = validated_deployment_svg()?;
    let document = Document::parse(&svg).map_err(|error| error.to_string())?;
    for expected in ["Customer's mobile device", "[Apple iOS or Android]"] {
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("text") && text(*node) == expected)
            .ok_or("deployment label missing from original")?;
        let mut changed = svg.clone();
        changed.replace_range(label.range(), "");
        assert_eq!(
            GeometryContract::deployment(&changed).err().as_deref(),
            Some("semantic element must be unique")
        );
    }
    Ok(())
}

#[test]
fn c4_deployment_rejects_missing_outer_rectangle() -> Result<(), String> {
    let svg = validated_deployment_svg()?;
    let changed = remove_non_mobile_rectangles(&svg)?;
    assert_eq!(
        GeometryContract::deployment(&changed).err().as_deref(),
        Some("deployment parent geometry missing")
    );
    Ok(())
}

fn remove_non_mobile_rectangles(svg: &str) -> Result<String, String> {
    let document = Document::parse(svg).map_err(|error| error.to_string())?;
    let mobile = document
        .descendants()
        .find(|node| {
            node.attribute("id")
                .is_some_and(|id| id.ends_with("-mobile"))
        })
        .ok_or("mobile container missing")?;
    let inner = mobile
        .children()
        .find(|node| node.has_tag_name("rect"))
        .ok_or("mobile rectangle missing")?;
    let ranges: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("rect") && *node != inner)
        .map(|node| node.range())
        .collect();
    if ranges.is_empty() {
        return Err("outer rectangles missing from valid SVG".to_owned());
    }
    let mut changed = svg.to_owned();
    for range in ranges.into_iter().rev() {
        changed.replace_range(range, "");
    }
    Ok(changed)
}

fn validated_deployment_svg() -> Result<String, String> {
    let svg = render_mermaid_svg(C4_DEPLOYMENT_SOURCE)?;
    GeometryContract::deployment(&svg)?;
    Ok(svg)
}
