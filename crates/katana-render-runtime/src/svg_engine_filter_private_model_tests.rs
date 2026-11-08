use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use super::super::{Options, svgtree};
use super::{
    Color, EId, FilterChildContext, FilterResults, Kind, LightSource, NonZeroRect, Size, SvgNode,
    Units, append_filter_child, collect_children, convert_filter_function, convert_light_source,
    convert_lighting_child, converter,
};

#[path = "svg_engine_filter_id_transfer_tests.rs"]
mod id_transfer_tests;

const REGION_SIZE: f32 = 100.0;
const LIGHT_X: f32 = 2.0;
const LIGHT_Y: f32 = 3.0;
const LIGHT_Z: f32 = 4.0;
const AZIMUTH: f32 = 30.0;
const ELEVATION: f32 = 40.0;

fn with_fixture(
    source: &str,
    run: impl FnOnce(
        SvgNode<'_, '_>,
        &converter::State<'_>,
        &mut converter::Cache,
    ) -> Result<(), String>,
) -> Result<(), String> {
    let xml = roxmltree::Document::parse(source).map_err(|error| error.to_string())?;
    let document = svgtree::Document::parse_tree(&xml, None).map_err(|error| error.to_string())?;
    let node = document
        .element_by_id("target")
        .ok_or("target is missing")?;
    let options = Options::default();
    let fontdb = Rc::new(RefCell::new(options.fontdb.clone()));
    let state = converter::State {
        parent_clip_path: None,
        parent_markers: Vec::new(),
        context_element: None,
        fe_image_link: false,
        view_box: region()?,
        use_size: (None, None),
        opt: &options,
        fontdb: fontdb.clone(),
    };
    let mut cache = converter::Cache::new(options.fontdb.clone(), fontdb);
    run(node, &state, &mut cache)
}

fn region() -> Result<NonZeroRect, String> {
    NonZeroRect::from_xywh(0.0, 0.0, REGION_SIZE, REGION_SIZE)
        .ok_or_else(|| "invalid fixture region".to_owned())
}

#[test]
fn filter_function_rejects_url_and_accepts_saturate() -> Result<(), String> {
    with_fixture(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="target"/></svg>"#,
        |node, state, _| {
            assert!(
                convert_filter_function(node, state, svgtypes::FilterValue::Url("sample"))
                    .is_none()
            );
            assert!(matches!(
                convert_filter_function(node, state, svgtypes::FilterValue::Saturate(1.0)),
                Some(Kind::ColorMatrix(_))
            ));
            Ok(())
        },
    )
}

#[test]
fn bbox_units_require_bbox_before_collecting_filter_children() -> Result<(), String> {
    with_fixture(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><filter id="target"><feFlood/></filter></svg>"#,
        |node, state, cache| {
            let missing = collect_children(
                &node,
                Units::ObjectBoundingBox,
                state,
                None,
                region()?,
                cache,
            );
            assert!(missing.is_empty());
            let present = collect_children(
                &node,
                Units::ObjectBoundingBox,
                state,
                Some(region()?),
                region()?,
                cache,
            );
            assert_eq!(present.len(), 1);
            assert!(matches!(present[0].kind, Kind::Flood(_)));
            Ok(())
        },
    )
}

#[test]
fn append_text_node_is_noop_and_flood_is_appended() -> Result<(), String> {
    with_fixture(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="target">real text</text><filter><feFlood id="flood"/></filter></svg>"#,
        |node, state, cache| {
            let text = node
                .children()
                .find(|child| child.is_text())
                .ok_or("text node missing")?;
            let flood = node
                .document()
                .element_by_id("flood")
                .ok_or("flood missing")?;
            let mut context = FilterChildContext {
                units: Units::UserSpaceOnUse,
                state,
                object_bbox: None,
                filter_region: region()?,
                scale: Size::from_wh(1.0, 1.0).ok_or("invalid unit scale")?,
                cache,
            };
            assert_text_then_flood(text, flood, &mut context);
            Ok(())
        },
    )
}

fn assert_text_then_flood(
    text: SvgNode<'_, '_>,
    flood: SvgNode<'_, '_>,
    context: &mut FilterChildContext<'_, '_>,
) {
    let mut primitives = Vec::new();
    let mut results = FilterResults {
        names: HashSet::new(),
        idx: 1,
    };
    assert!(append_filter_child(
        text,
        context,
        &mut primitives,
        &mut results
    ));
    assert!(primitives.is_empty());
    assert_eq!(results.idx, 1);
    assert!(results.names.is_empty());
    assert!(append_filter_child(
        flood,
        context,
        &mut primitives,
        &mut results
    ));
    assert_eq!(primitives.len(), 1);
    assert!(matches!(primitives[0].kind, Kind::Flood(_)));
}

#[test]
fn unsupported_lighting_tag_returns_transparent_flood_with_diffuse_control() -> Result<(), String> {
    with_fixture(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><filter><feDiffuseLighting id="target"><feDistantLight azimuth="30" elevation="40"/></feDiffuseLighting></filter></svg>"#,
        |node, _, _| {
            match convert_lighting_child(node, EId::FeFlood, &[]) {
                Kind::Flood(flood) => {
                    assert_eq!(flood.color, Color::black());
                    assert_eq!(flood.opacity.get(), 0.0);
                }
                _ => {
                    return Err(
                        "unsupported lighting did not produce a transparent flood".to_owned()
                    );
                }
            }
            assert!(matches!(
                convert_lighting_child(node, EId::FeDiffuseLighting, &[]),
                Kind::DiffuseLighting(_)
            ));
            Ok(())
        },
    )
}

fn light_source(
    children: &str,
    check: impl FnOnce(Option<LightSource>) -> Result<(), String>,
) -> Result<(), String> {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><filter><feDiffuseLighting id="target">{children}</feDiffuseLighting></filter></svg>"#
    );
    with_fixture(&source, |node, _, _| check(convert_light_source(node)))
}

fn assert_point(light: Option<LightSource>) -> Result<(), String> {
    let Some(LightSource::PointLight(point)) = light else {
        return Err("expected point light".to_owned());
    };
    assert_eq!((point.x, point.y, point.z), (LIGHT_X, LIGHT_Y, LIGHT_Z));
    Ok(())
}

fn assert_distant(light: Option<LightSource>) -> Result<(), String> {
    let Some(LightSource::DistantLight(distant)) = light else {
        return Err("expected distant light".to_owned());
    };
    assert_eq!((distant.azimuth, distant.elevation), (AZIMUTH, ELEVATION));
    Ok(())
}

#[test]
fn light_sources_preserve_authored_attributes() -> Result<(), String> {
    light_source(
        r#"<feDistantLight azimuth="30" elevation="40"/>"#,
        assert_distant,
    )?;
    light_source(r#"<fePointLight x="2" y="3" z="4"/>"#, assert_point)?;
    light_source(
        r#"<feSpotLight x="2" y="3" z="4" pointsAtX="2" pointsAtY="3" pointsAtZ="4" specularExponent="2" limitingConeAngle="30"/>"#,
        |light| {
            let Some(LightSource::SpotLight(spot)) = light else {
                return Err("expected spot light".to_owned());
            };
            assert_eq!((spot.x, spot.y, spot.z), (LIGHT_X, LIGHT_Y, LIGHT_Z));
            assert_eq!(
                (spot.points_at_x, spot.points_at_y, spot.points_at_z),
                (LIGHT_X, LIGHT_Y, LIGHT_Z)
            );
            assert_eq!(spot.specular_exponent.get(), LIGHT_X);
            assert_eq!(spot.limiting_cone_angle, Some(AZIMUTH));
            Ok(())
        },
    )
}

#[test]
fn first_supported_light_wins_and_unsupported_children_are_skipped() -> Result<(), String> {
    light_source("<feFlood/>", |light| {
        assert!(light.is_none());
        Ok(())
    })?;
    light_source(
        r#"<feFlood/><fePointLight x="2" y="3" z="4"/>"#,
        assert_point,
    )?;
    light_source(
        r#"<feDistantLight azimuth="30" elevation="40"/><fePointLight x="2" y="3" z="4"/>"#,
        assert_distant,
    )?;
    light_source(
        r#"<fePointLight x="2" y="3" z="4"/><feDistantLight azimuth="30" elevation="40"/>"#,
        assert_point,
    )
}
