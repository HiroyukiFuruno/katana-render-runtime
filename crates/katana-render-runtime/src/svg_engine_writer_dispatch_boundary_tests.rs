use std::panic::{AssertUnwindSafe, catch_unwind};

use rstest::rstest;

use super::{
    WriteOptions, write_basic_filter_primitive, write_effect_filter_primitive,
    write_geometry_filter_primitive,
};
use crate::svg_engine::usvg::{self, filter};

const FILTER_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><filter id="f" filterUnits="userSpaceOnUse" x="0" y="0" width="8" height="8"><feFlood result="source"/><feMorphology in="source" operator="dilate" radius="1"/></filter></defs><rect width="8" height="8" filter="url(#f)"/></svg>"#;

#[derive(Clone, Copy, Debug)]
enum Dispatch {
    Basic,
    Geometry,
    Effect,
}

#[rstest]
#[case(Dispatch::Basic)]
#[case(Dispatch::Geometry)]
#[case(Dispatch::Effect)]
fn misrouted_filter_primitive_fails_fast(#[case] dispatch: Dispatch) -> Result<(), String> {
    let tree = usvg::Tree::from_str(FILTER_SVG, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let filter = tree.filters().first().ok_or("filter missing")?;
    let primitive = incompatible_primitive(filter, dispatch)?;
    assert_dispatch_panics(dispatch, primitive, filter)
}

fn incompatible_primitive(
    filter: &filter::Filter,
    dispatch: Dispatch,
) -> Result<&filter::Primitive, String> {
    filter.primitives().iter().find(|primitive| match dispatch {
        Dispatch::Basic | Dispatch::Geometry => {
            matches!(primitive.kind(), filter::Kind::Morphology(_))
        }
        Dispatch::Effect => matches!(primitive.kind(), filter::Kind::Flood(_)),
    }).ok_or_else(|| "unexpected filter primitive".to_owned())
}

fn assert_dispatch_panics(
    dispatch: Dispatch,
    primitive: &filter::Primitive,
    filter: &filter::Filter,
) -> Result<(), String> {
    let mut xml = xmlwriter::XmlWriter::new(xmlwriter::Options::default());
    let panic = catch_unwind(AssertUnwindSafe(|| {
        invoke_misrouted(dispatch, primitive, filter, &mut xml);
    }));
    let payload = panic.err().ok_or("misrouted primitive was accepted")?;
    let message = payload.downcast_ref::<&str>().copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .ok_or("panic payload was not a string")?;
    assert!(message.contains("internal error: entered unreachable code"), "{dispatch:?}: {message}");
    Ok(())
}

fn invoke_misrouted(
    dispatch: Dispatch,
    primitive: &filter::Primitive,
    filter: &filter::Filter,
    xml: &mut xmlwriter::XmlWriter,
) {
    match dispatch {
        Dispatch::Basic => write_basic_filter_primitive(primitive, filter, xml),
        Dispatch::Geometry => {
            write_geometry_filter_primitive(primitive, filter, &WriteOptions::default(), xml)
        }
        Dispatch::Effect => write_effect_filter_primitive(primitive, filter, xml),
    }
}
