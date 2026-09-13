use super::super::DrawioJsRuntimeOps;
use super::super::page_crop_test_support::temp_runtime_path;
use super::page_crop_bundle_fixtures::sourdough_multi_page_bundle;
use super::page_crop_geometry::{SvgBox, svg_canvas_box};
use super::page_crop_source_fixtures::sourdough_multi_page_source;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_trims_multi_page_device_bottom_gap_of_one_pixel() {
    let path = temp_runtime_path("krr-drawio-sourdough-bottom-minus-one-unit");
    assert!(std::fs::write(&path, sourdough_multi_page_bundle(562)).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        sourdough_multi_page_source(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg_canvas_box(svg) == Some(SvgBox::new(0, 0, 792, 562))),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_two_pixel_multi_page_device_bottom_gap() {
    let path = temp_runtime_path("krr-drawio-sourdough-bottom-minus-two-unit");
    assert!(std::fs::write(&path, sourdough_multi_page_bundle(561)).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        sourdough_multi_page_source(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg_canvas_box(svg) == Some(SvgBox::new(0, 0, 792, 563))),
        "{rendered:?}"
    );
}
