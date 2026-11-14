use super::*;

#[test]
fn geometry_contract_rejects_nonfinite_svg_viewbox() {
    for view_box in ["0 0 NaN 100", "1e309 0 100 100", "0 0 100 -1e309"] {
        let svg = format!(r#"<svg viewBox="{view_box}"><rect width="10" height="10"/></svg>"#);
        assert_eq!(
            document(&svg).err().as_deref(),
            Some("viewBox contains an invalid number"),
            "{view_box}"
        );
    }
}

pub(super) const ARCH: &str = r#"<svg viewBox="-20 -20 400 200"><g class="architecture-services"><g id="id-service-a" class="architecture-service"><g transform="translate(40,70)"><text>A</text></g><svg width="80" height="80" viewBox="0 0 80 80"/></g><g id="id-service-b" class="architecture-service" transform="translate(200,0)"><g transform="translate(40,70)"><text>B</text></g><svg width="80" height="80" viewBox="0 0 80 80"/></g></g><g class="architecture-edges"><path class="edge" id="id-L_a_b_0" d="M80 40L200 40"/></g><g class="architecture-groups"><rect id="id-group-app" x="-10" y="-10" width="300" height="100"/><text>Group</text></g></svg>"#;
pub(super) const C4: &str = r#"<svg viewBox="0 0 300 100"><text x="0" y="0">Dynamic diagram for API Application</text><g class="node c4-container" id="id-spa"><rect class="label-container" width="80" height="80"/><g class="c4-name"><text x="40" y="20">Single Page Application</text></g><g class="c4-type"><text x="40" y="35">javascript and react</text></g></g><g transform="translate(200,0)"><g class="node c4-container" id="id-api"><rect class="label-container" width="80" height="80"/><g class="c4-name"><text x="40" y="20">API Application</text></g><g class="c4-type"><text x="40" y="35">Java and Spring Boot</text></g></g></g><line x1="80" y1="40" x2="200" y2="40" marker-end="url(#arrow)"/><text x="140" y="40">1: Uses</text><text x="140" y="55">[JSON/HTTPS]</text></svg>"#;
const TREEMAP_SOURCE_VALUES: &[(&str, i32)] = &[
    ("Mermaid", 45),
    ("DOM shim", 25),
    ("Rasterize", 20),
    ("Cache", 10),
];

pub(super) fn architecture_pair(svg: &str) -> Result<(), String> {
    GeometryContract::architecture(
        svg,
        &[("a", "A"), ("b", "B")],
        &[("a", "b", 'R', 'L')],
        ("app", "Group"),
    )
}

#[test]
fn geometry_contract_rejects_architecture_missing_links_collapsed_nodes_and_detached_labels()
-> Result<(), String> {
    architecture_pair(ARCH)?;
    for (old, new) in [
        ("id-L_a_b_0", "id-other-path"),
        ("translate(200,0)", "translate(0,0)"),
        (">B</text>", ">A</text>"),
        ("translate(40,70)", "translate(40,700)"),
        ("-20 -20 400 200", "0 0 1 1"),
        (r#"id-service-b""#, r#"id-service-b" opacity="0""#),
    ] {
        assert!(
            architecture_pair(&ARCH.replacen(old, new, 1)).is_err(),
            "corruption {old} accepted"
        );
    }
    Ok(())
}

#[test]
fn geometry_contract_rejects_architecture_group_escape_missing_edges_and_invalid_ports()
-> Result<(), String> {
    architecture_pair(ARCH)?;
    let escaped_group = ARCH.replacen(r#"width="300""#, r#"width="100""#, 1);
    assert!(architecture_pair(&escaped_group).is_err());

    let missing_edge = ARCH.replace(
        r#"<path class="edge" id="id-L_a_b_0" d="M80 40L200 40"/>"#,
        "",
    );
    assert!(architecture_pair(&missing_edge).is_err());

    assert!(
        GeometryContract::architecture(
            ARCH,
            &[("a", "A"), ("b", "B")],
            &[("a", "b", 'X', 'L')],
            ("app", "Group"),
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn geometry_contract_checks_architecture_vertical_ports_in_either_path_direction() {
    for (path, expected) in [
        ("M40 80L240 0", None),
        ("M240 0L40 80", None),
        (
            "M40 79L240 0",
            Some("edge endpoints detached from source ports"),
        ),
    ] {
        let svg = ARCH.replace("M80 40L200 40", path);
        assert_eq!(
            GeometryContract::architecture(
                &svg,
                &[("a", "A"), ("b", "B")],
                &[("a", "b", 'B', 'T')],
                ("app", "Group"),
            )
            .err()
            .as_deref(),
            expected
        );
    }
}

#[test]
fn geometry_contract_rejects_c4_detached_endpoints_missing_relation_and_wrong_node_labels()
-> Result<(), String> {
    GeometryContract::dynamic(C4, false)?;
    for (old, new) in [
        (r#"x2="200""#, r#"x2="198""#),
        ("1: Uses", "no relation"),
        ("JSON/HTTPS", "other technology"),
        ("Single Page Application", "API Application"),
        ("translate(200,0)", "translate(0,0)"),
        (r#"x="140""#, r#"x="290""#),
    ] {
        assert!(
            GeometryContract::dynamic(&C4.replacen(old, new, 1), false).is_err(),
            "corruption {old} accepted"
        );
    }
    Ok(())
}

#[test]
fn geometry_contract_accepts_c4_vertical_edges_and_rejects_interior_endpoint() -> Result<(), String>
{
    let vertical_edges = C4.replace(
        r#"x1="80" y1="40" x2="200" y2="40""#,
        r#"x1="40" y1="80" x2="240" y2="0""#,
    );
    GeometryContract::dynamic(&vertical_edges, false)?;

    let detached_start = vertical_edges.replace(r#"y1="80""#, r#"y1="79""#);
    assert!(GeometryContract::dynamic(&detached_start, false).is_err());
    Ok(())
}

pub(super) fn treemap_control() -> String {
    let mut svg = r#"<svg viewBox="0 0 220 100"><text>Runtime cost</text>"#.to_owned();
    for (index, (label, value)) in TREEMAP_SOURCE_VALUES.iter().enumerate() {
        svg.push_str(&format!(r#"<g class="treemapLeafGroup" transform="translate({},0)"><rect class="treemapLeaf" width="50" height="80"/><text class="treemapLabel" x="25" y="20">{label}</text><text class="treemapValue" x="25" y="35" style="font-size: 23px; fill: lightgrey">{value}</text></g>"#,index*55));
    }
    svg.push_str("</svg>");
    svg
}

#[test]
fn geometry_contract_rejects_missing_or_empty_treemap_value_style() -> Result<(), String> {
    let svg = treemap_control();
    GeometryContract::treemap(&svg)?;
    let style = r#" style="font-size: 23px; fill: lightgrey""#;
    let missing = svg.replacen(style, "", 1);
    let empty = svg.replacen(style, r#" style="  ""#, 1);
    assert_eq!(
        GeometryContract::treemap(&missing),
        Err("missing style".into())
    );
    assert_eq!(
        GeometryContract::treemap(&empty),
        Err("treemap value style missing".into())
    );
    Ok(())
}

#[test]
fn geometry_contract_rejects_treemap_wrong_values_detached_labels_and_collapsed_leaf_boxes()
-> Result<(), String> {
    let svg = treemap_control();
    GeometryContract::treemap(&svg)?;
    for (old, new) in [
        (">45</text>", ">10</text>"),
        ("Runtime cost", "wrong title"),
        (r#"width="50""#, r#"width="0""#),
        (r#"x="25""#, r#"x="500""#),
        ("translate(55,0)", "translate(0,0)"),
        ("treemapLeafGroup", "unrelatedGroup"),
    ] {
        assert!(
            GeometryContract::treemap(&svg.replacen(old, new, 1)).is_err(),
            "corruption {old} accepted"
        );
    }
    Ok(())
}

const GIT: &str = r#"<svg viewBox="0 0 150 100"><circle class="commit base" cx="20" cy="20" r="10"/><circle class="commit rust-js" cx="60" cy="60" r="10"/><circle class="commit merged" cx="100" cy="20" r="10"/><g><rect class="commit-label-bkg" x="10" y="25" width="30" height="20"/><text class="commit-label" x="12" y="30">base</text></g><g><rect class="commit-label-bkg" x="45" y="65" width="30" height="20"/><text class="commit-label" x="47" y="70">rust-js</text></g><g class="branchLabel"><text>main</text></g><g class="branchLabel"><text>feature</text></g><path class="arrow" d="M20 20L60 60"/><path class="arrow" d="M20 20L100 20"/><path class="arrow" d="M60 60L100 20"/></svg>"#;

#[test]
fn geometry_contract_rejects_git_missing_relationships_and_misassociated_commit_labels()
-> Result<(), String> {
    GeometryContract::git_graph(GIT)?;
    for (old, new) in [
        (r#"class="arrow""#, r#"class="other""#),
        ("M20 20L100 20", "M20 20L60 60"),
        (r#"cx="100""#, r#"cx="20""#),
        (r#"x="47""#, r#"x="12""#),
    ] {
        assert!(
            GeometryContract::git_graph(&GIT.replacen(old, new, 1)).is_err(),
            "corruption {old} accepted"
        );
    }
    Ok(())
}

const INVALID_GIT_COMMIT_GEOMETRY: &[(&str, &str, &str)] = &[
    (
        r#"<g class="branchLabel"><text>feature</text></g>"#,
        "",
        "semantic element must be unique",
    ),
    (
        r#"<g class="branchLabel"><text>feature</text></g>"#,
        r#"<g class="branchLabel"><text>main</text></g>"#,
        "semantic element must be unique",
    ),
    (
        r#"class="commit base" cx="20" cy="20" r="10""#,
        r#"class="commit base" cx="20" cy="20" r="0""#,
        "commit bullet must have area",
    ),
    (
        r#"class="commit merged" cx="100" cy="20""#,
        r#"class="commit merged" cx="25" cy="35""#,
        "commit label nearer a different source commit",
    ),
    (
        "</svg>",
        r#"<circle class="commit extra" cx="145" cy="95" r="3"/></svg>"#,
        "git commit inventory changed",
    ),
];

#[test]
fn geometry_contract_rejects_invalid_git_commit_geometry() -> Result<(), String> {
    GeometryContract::git_graph(GIT)?;
    for (original, replacement, expected) in INVALID_GIT_COMMIT_GEOMETRY {
        let svg = GIT.replacen(*original, replacement, 1);
        assert_eq!(
            GeometryContract::git_graph(&svg).err().as_deref(),
            Some(*expected)
        );
    }
    Ok(())
}

fn shared_git_labels() -> String {
    GIT.replace("base</text></g><g><rect", "base</text><rect")
        .replacen("<g><rect", "<g class=\"commit-labels\"><rect", 1)
}

#[test]
fn geometry_contract_pairs_each_git_label_in_shared_multi_label_group() -> Result<(), String> {
    let svg = shared_git_labels();
    GeometryContract::git_graph(&svg)?;
    for (old, new) in [
        (
            r#"<text class="commit-label" x="47""#,
            r#"<circle/><text class="commit-label" x="47""#,
        ),
        ("commit-label-bkg", "unrelated-background"),
        ("base</text>", "rust-js</text>"),
    ] {
        assert!(
            GeometryContract::git_graph(&svg.replacen(old, new, 1)).is_err(),
            "corruption {old} accepted"
        );
    }
    Ok(())
}

#[test]
fn geometry_contract_requires_source_dynamic_title_in_both_languages() -> Result<(), String> {
    let japanese = C4
        .replace(
            "Dynamic diagram for API Application",
            "APIアプリケーションのダイナミック図",
        )
        .replace("Single Page Application", "シングルページアプリケーション")
        .replace("API Application", "APIアプリケーション")
        .replace("javascript and react", "JavaScript と React")
        .replace("Java and Spring Boot", "Java と Spring Boot")
        .replace("Uses", "利用する");
    for (svg, japanese, title) in [
        (C4, false, "Dynamic diagram for API Application"),
        (
            japanese.as_str(),
            true,
            "APIアプリケーションのダイナミック図",
        ),
    ] {
        GeometryContract::dynamic(svg, japanese)?;
        for changed in ["", "unrelated title"] {
            assert!(GeometryContract::dynamic(&svg.replace(title, changed), japanese).is_err());
        }
    }
    Ok(())
}
