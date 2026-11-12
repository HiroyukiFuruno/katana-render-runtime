use super::super::{document, endpoints, global, label_in_box, number, rectangle, view_contains};
#[path = "js_runtime_geometry_boundary_test_support.rs"]
mod support;
use support::{element, first_element};

#[test]
fn fixture_helpers_reject_malformed_xml_and_missing_tags() -> Result<(), String> {
    assert!(element("<svg>").is_err());
    let parsed = element("<svg/>")?;
    assert!(first_element(&parsed, "path").is_err());
    Ok(())
}

#[test]
fn malformed_documents_and_required_viewbox_are_rejected() {
    for svg in [
        "<svg>",
        "<html viewBox='0 0 1 1'/>",
        "<svg/>",
        "<svg viewBox='0 0 1'/>",
    ] {
        assert!(document(svg).is_err(), "invalid document accepted: {svg}");
    }
    assert!(document("<svg viewBox='0 0 10 10'/>").is_ok());
}

#[test]
fn missing_invalid_and_nonfinite_dimensions_are_rejected() -> Result<(), String> {
    for (attributes, expected) in [
        ("height='5'", "missing width"),
        ("width='NaN' height='5'", "non-finite geometry"),
    ] {
        let svg = format!("<svg><rect {attributes}/></svg>");
        let parsed = element(&svg)?;
        let rect = first_element(&parsed, "rect")?;
        let error = rectangle(rect).err().ok_or("invalid rectangle accepted")?;
        assert!(error.contains(expected), "unexpected error: {error}");
    }
    let valid = element("<svg><rect width='2' height='5'/></svg>")?;
    assert_eq!(
        rectangle(first_element(&valid, "rect")?)?,
        [0.0, 0.0, 2.0, 5.0]
    );
    let parsed = element("<svg><rect width='oops' height='5'/></svg>")?;
    let rect = first_element(&parsed, "rect")?;
    assert!(number(rect, "width", None).is_err());
    Ok(())
}

#[test]
fn rectangles_reject_invalid_origin_and_missing_height() -> Result<(), String> {
    for (attributes, expected) in [
        ("x='invalid' width='2' height='5'", "invalid float literal"),
        ("y='NaN' width='2' height='5'", "non-finite geometry"),
        ("width='2'", "missing height"),
    ] {
        let markup = format!("<svg><rect {attributes}/></svg>");
        let svg = element(&markup)?;
        let error = rectangle(first_element(&svg, "rect")?)
            .err()
            .ok_or("invalid rectangle accepted")?;
        assert!(error.contains(expected), "unexpected error: {error}");
    }
    Ok(())
}

#[test]
fn label_box_without_text_and_path_without_data_are_rejected() -> Result<(), String> {
    let svg = element("<svg><g><rect width='2' height='2'/></g><path/></svg>")?;
    assert_eq!(
        label_in_box(first_element(&svg, "g")?, [0.0, 0.0, 2.0, 2.0]),
        Err("associated text element required".into())
    );
    assert!(endpoints(first_element(&svg, "path")?).is_err());
    Ok(())
}

#[test]
fn invalid_transform_and_overflow_do_not_produce_geometry() -> Result<(), String> {
    for transform in ["translate(invalid)", "matrix(1e308 0 0 1e308 0 0)"] {
        let svg = format!("<svg><g transform='{transform}'><rect/></g></svg>");
        let parsed = element(&svg)?;
        let rect = first_element(&parsed, "rect")?;
        assert!(
            global(rect, [2.0, 0.0]).is_err(),
            "transform accepted: {transform}"
        );
    }
    let valid = element("<svg><g transform='translate(3 4)'><rect/></g></svg>")?;
    assert_eq!(
        global(first_element(&valid, "rect")?, [2.0, 0.0])?,
        [5.0, 4.0]
    );
    Ok(())
}

#[test]
fn malformed_paths_missing_viewbox_and_empty_labels_are_rejected() -> Result<(), String> {
    for path in ["M0 0", "M0 0 L0 0", "M0 0 Lbroken"] {
        let svg = format!("<svg><path d='{path}'/></svg>");
        let parsed = element(&svg)?;
        assert!(endpoints(first_element(&parsed, "path")?).is_err());
    }
    let valid = element("<svg><path d='M0 0 L1 1'/></svg>")?;
    assert_eq!(
        endpoints(first_element(&valid, "path")?)?,
        ([0.0, 0.0], [1.0, 1.0])
    );
    let parsed = element("<svg><g><text/></g></svg>")?;
    let root = parsed.root_element();
    assert!(view_contains(root, [0.0, 0.0, 1.0, 1.0]).is_err());
    let group = first_element(&parsed, "g")?;
    assert_eq!(
        label_in_box(group, [0.0, 0.0, 1.0, 1.0]),
        Err("associated label must contain text".to_string())
    );
    Ok(())
}

#[test]
fn closed_subpath_preserves_the_explicit_endpoints_of_the_full_path() -> Result<(), String> {
    let svg = element("<svg><path d='M0 0 L4 0 L4 4 Z M6 6 L9 7'/></svg>")?;
    let path = first_element(&svg, "path")?;
    assert_eq!(endpoints(path)?, ([0.0, 0.0], [9.0, 7.0]));
    Ok(())
}

#[test]
fn rectangles_reject_zero_and_negative_area() -> Result<(), String> {
    for (width, height) in [("0", "5"), ("-1", "5"), ("5", "0"), ("5", "-1")] {
        let source = format!("<svg><rect width='{width}' height='{height}'/></svg>");
        let svg = element(&source)?;
        let rect = first_element(&svg, "rect")?;
        assert_eq!(
            rectangle(rect),
            Err("semantic rectangle must have area".into())
        );
    }
    Ok(())
}

#[test]
fn viewbox_containment_checks_both_axes_with_serialized_tolerance() -> Result<(), String> {
    let svg = element("<svg viewBox='0 0 10 10'/>")?;
    let root = svg.root_element();
    assert!(view_contains(root, [-0.000009, 0.0, 10.000009, 10.000009]).is_ok());
    assert!(view_contains(root, [0.0, -0.00002, 10.0, 10.0]).is_err());
    assert!(view_contains(root, [0.0, 0.0, 10.0, 10.00002]).is_err());
    Ok(())
}

#[test]
fn hidden_ancestors_reject_global_geometry() -> Result<(), String> {
    for visibility in ["display='none'", "visibility='hidden'", "opacity='0'"] {
        let source = format!("<svg><g {visibility}><rect/></g></svg>");
        let svg = element(&source)?;
        let rect = first_element(&svg, "rect")?;
        assert!(global(rect, [1.0, 2.0]).is_err(), "accepted {visibility}");
    }
    Ok(())
}

#[test]
fn path_endpoints_apply_nested_transforms() -> Result<(), String> {
    let svg = element(
        "<svg><g transform='translate(10 20)'><path transform='scale(2 3)' d='M1 2 L4 5'/></g></svg>",
    )?;
    assert_eq!(
        endpoints(first_element(&svg, "path")?)?,
        ([12.0, 26.0], [18.0, 35.0])
    );
    Ok(())
}
