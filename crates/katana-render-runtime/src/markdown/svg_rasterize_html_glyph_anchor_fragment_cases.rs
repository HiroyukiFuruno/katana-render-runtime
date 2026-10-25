use resvg::usvg::{self, Node, Options, Tree};
use std::sync::Arc;

pub(super) struct AlignmentNegativeCase {
    pub(super) name: &'static str,
    pub(super) attributes: &'static str,
    pub(super) content: &'static str,
    pub(super) xml_space: &'static str,
    pub(super) extra_root: &'static str,
    pub(super) ready_for_paint: bool,
}

pub(super) fn parse_prepare_fixture(
    database: &Arc<usvg::fontdb::Database>,
    attributes: &str,
    content: &str,
    xml_space: &str,
    extra_root: &str,
    use_source_context: bool,
) -> Result<(Tree, super::super::Prepared), String> {
    let source = super::fixture_source(attributes, content, xml_space, extra_root);
    let tree = Tree::from_str(
        &source,
        &Options {
            fontdb: Arc::clone(database),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("anchor fragment fixture text missing".to_owned());
    };
    let prepared = prepare_text(database, text, &source, use_source_context)?;
    Ok((tree, prepared))
}

fn prepare_text(
    database: &Arc<usvg::fontdb::Database>,
    text: &usvg::Text,
    source: &str,
    use_source_context: bool,
) -> Result<super::super::Prepared, String> {
    let context = use_source_context
        .then(|| {
            super::super::super::provenance::authored_svg_context(
                source,
                &super::super::super::GeneratedClipProvenance::default(),
                false,
            )
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    let rasterizer = if let Some(context) = &context {
        super::super::HtmlGlyphRasterizer::with_resvg_text_and_source_context(
            database,
            std::collections::HashSet::new(),
            &context.text,
        )
    } else {
        super::super::HtmlGlyphRasterizer::new(database)
    };
    rasterizer.prepare(text)
}

pub(super) const WORD_TSPAN_SPLIT: &str =
    "<tspan>Internet </tspan><tspan>Banking </tspan><tspan>System</tspan>";
pub(super) const WORD_TSPAN_VARIANTS: &[&str] = &[
    WORD_TSPAN_SPLIT,
    "<tspan>Internet</tspan><tspan> Banking</tspan><tspan> System</tspan>",
];

pub(super) const ALIGNMENT_NEGATIVE_CASES: &[AlignmentNegativeCase] = &[
    AlignmentNegativeCase {
        name: "mixed font weight",
        attributes: "",
        content: "<tspan>Internet </tspan><tspan>Banking </tspan><tspan font-weight=\"700\">System</tspan>",
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "mixed font size",
        attributes: "",
        content: "<tspan>Internet </tspan><tspan>Banking </tspan><tspan font-size=\"15\">System</tspan>",
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "mixed baseline shift",
        attributes: "",
        content: "<tspan>Internet </tspan><tspan>Banking </tspan><tspan baseline-shift=\"2\">System</tspan>",
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "letter spacing",
        attributes: "letter-spacing=\"0.25\"",
        content: WORD_TSPAN_SPLIT,
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "textLength",
        attributes: "textLength=\"200\"",
        content: WORD_TSPAN_SPLIT,
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "textLength with normalized whitespace",
        attributes: "textLength=\"200\"",
        content: "<tspan>Internet</tspan>  <tspan>Banking </tspan><tspan>System</tspan>",
        xml_space: "default",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "normalized source text mismatch",
        attributes: "",
        content: "<tspan>Internet</tspan>  <tspan>Banking </tspan><tspan>System</tspan>",
        xml_space: "default",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "duplicate source text root",
        attributes: "",
        content: WORD_TSPAN_SPLIT,
        xml_space: "preserve",
        extra_root: "<text xml:space=\"preserve\" x=\"120\" y=\"32\" transform=\"scale(0.99999434)\" font-family=\"Arial\" font-size=\"14\" text-anchor=\"middle\"><tspan>Internet </tspan><tspan>Banking </tspan><tspan>System</tspan></text>",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "multiple chunks",
        attributes: "",
        content: "<tspan>Internet </tspan><tspan x=\"120\">Banking </tspan><tspan>System</tspan>",
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "dx positioning",
        attributes: "dx=\"1\"",
        content: WORD_TSPAN_SPLIT,
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "rotation",
        attributes: "rotate=\"1\"",
        content: WORD_TSPAN_SPLIT,
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "nonzero trailing dy",
        attributes: "dy=\"0 2\"",
        content: WORD_TSPAN_SPLIT,
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: true,
    },
    AlignmentNegativeCase {
        name: "text decoration",
        attributes: "text-decoration=\"underline\"",
        content: WORD_TSPAN_SPLIT,
        xml_space: "preserve",
        extra_root: "",
        ready_for_paint: false,
    },
];
