use super::preprocess_for_rasterizer;
use resvg::usvg::roxmltree::Document;

const PREFIX: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><switch>"#;
const SUFFIX: &str = r#"<text id="fallback" x="0" y="12">Fallback</text></switch><path id="tail" d="M0 20H40"/></svg>"#;
const FOREIGN_OBJECTS: &[(&str, &str)] = &[
    ("self-closing", "<foreignObject/>"),
    (
        "child-self-closing",
        "<foreignObject><div><br/></div></foreignObject>",
    ),
    (
        "later-self-closing",
        "<foreignObject><div>content</div></foreignObject>",
    ),
    (
        "quoted-delimiter",
        r#"<foreignObject data-note="quoted /> delimiter"><div>content</div></foreignObject>"#,
    ),
    (
        "nested",
        "<foreignObject><foreignObject><div>nested</div></foreignObject></foreignObject>",
    ),
    (
        "namespace-prefix",
        r#"<s:foreignObject xmlns:s="http://www.w3.org/2000/svg"><div>content</div></s:foreignObject>"#,
    ),
];

#[test]
fn foreign_object_removal_preserves_complete_xml_and_svg_fallback() -> Result<(), String> {
    let expected = format!("{PREFIX}{SUFFIX}");
    for (label, fragment) in FOREIGN_OBJECTS {
        let input = format!("{PREFIX}{fragment}{SUFFIX}");
        Document::parse(&input).map_err(|error| format!("{label} input: {error}"))?;
        let output = preprocess_for_rasterizer(&input);
        assert_eq!(output, expected, "{label}");
        Document::parse(&output).map_err(|error| format!("{label} output: {error}"))?;
    }
    Ok(())
}

#[test]
fn foreign_object_removal_preserves_non_svg_namespaces_and_lookalikes() {
    let fragments = [
        r#"<h:foreignObject xmlns:h="http://www.w3.org/1999/xhtml">keep</h:foreignObject>"#,
        "<foreignObjectLike>keep</foreignObjectLike>",
        "<!-- <foreignObject/> -->",
    ];
    for fragment in fragments {
        let input = format!("{PREFIX}{fragment}{SUFFIX}");
        assert_eq!(preprocess_for_rasterizer(&input), input);
    }
}
