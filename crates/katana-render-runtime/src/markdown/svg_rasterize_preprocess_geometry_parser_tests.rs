use super::{FIRST_STROKE, LAST_STROKE, UNRELATED_FILL, parse_usvg_paints};
use crate::svg_engine::usvg::{Options, Tree};

const FILL_PRIORITY_CASES: [(&str, super::Rgb); 10] = [
    ("fill:#112233!important;fill:#334455", FIRST_STROKE),
    ("fill:#112233;fill:#334455!important", LAST_STROKE),
    ("fill:#112233!important;fill:#334455!important", LAST_STROKE),
    ("fill:#112233; malformed; fill:#334455", LAST_STROKE),
    ("*fill:#112233; malformed; fill:#334455", LAST_STROKE),
    ("*fill:#112233; malformed; invalid:value", FIRST_STROKE),
    (
        "fill:#112233!important; malformed; fill:#334455",
        FIRST_STROKE,
    ),
    (
        "fill:#112233; invalid:&quot;a;b&quot;; malformed; fill:#334455",
        LAST_STROKE,
    ),
    (
        "fill:#112233; invalid:fn(a;b); malformed; fill:#334455",
        LAST_STROKE,
    ),
    ("fill:#112233; /*a;b*/ malformed; fill:#334455", LAST_STROKE),
];

#[test]
fn public_parser_respects_fill_priority_and_keeps_independent_stroke() -> Result<(), String> {
    for (style, expected) in FILL_PRIORITY_CASES {
        let svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='12' height='12'><path d='M1 1 H11 V11 Z' stroke='#667788' style='{style}'/></svg>"
        );
        let (fill, stroke) = parse_usvg_paints(&svg)?;
        assert_eq!(fill, Some(expected), "{style}");
        assert_eq!(stroke, Some(UNRELATED_FILL), "{style}");
    }
    Ok(())
}

#[test]
fn public_parser_ignores_foreign_attributes_and_elements() -> Result<(), String> {
    let svg = "<svg xmlns='http://www.w3.org/2000/svg' xmlns:x='urn:foreign' width='12' height='12'><x:path d='M0 0 H1 V1 Z' fill='#334455'/><path d='M1 1 H11 V11 Z' x:fill='#334455' fill='#112233' x:unknown='value'/></svg>";
    let (fill, stroke) = parse_usvg_paints(svg)?;
    assert_eq!(fill, Some(FIRST_STROKE));
    assert_eq!(stroke, None);
    Ok(())
}

#[test]
fn public_parser_rejects_invalid_document_roots() {
    for svg in ["<path/>", "<svg", "<svg></path>"] {
        assert!(Tree::from_str(svg, &Options::default()).is_err(), "{svg}");
    }
}
