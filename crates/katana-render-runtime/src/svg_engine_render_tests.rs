use super::{resvg, usvg, writer_tests::upstream_writer_font_database};
use rstest::rstest;
use std::path::{Path, PathBuf};
use tiny_skia::{Pixmap, Transform};

#[path = "svg_engine_render_browser_reference_tests.rs"]
mod browser_reference_tests;
#[path = "svg_engine_render_compare_tests.rs"]
mod compare;
#[path = "svg_engine_render_context_tests.rs"]
mod context_paint_tests;
#[path = "svg_engine_render_diagnostic_tests.rs"]
mod diagnostic;
#[path = "svg_engine_render_failure_artifact_tests.rs"]
mod failure_artifact;
#[path = "svg_engine_render_input_contract_tests.rs"]
mod input_contract;
#[path = "svg_engine_render_inventory_tests.rs"]
mod inventory;
#[path = "svg_engine_render_marker_tests.rs"]
mod marker;
#[path = "svg_engine_render_morphology_color_tests.rs"]
mod morphology_color_tests;
#[path = "svg_engine_render_morphology_contract_tests.rs"]
mod morphology_contract;
#[path = "svg_engine_public_getter_tests.rs"]
mod public_getters;
const NORMAL_WIDTH: u32 = 300;
const EXTRA_FILTER_SCALE: f32 = 10.0;

#[rstest]
fn upstream_normal_render_matches_reference(
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/filters/**/*.svg")]
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/masking/**/*.svg")]
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/paint-servers/**/*.svg")]
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/shapes/**/*.svg")]
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/structure/**/*.svg")]
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/text/**/*.svg")]
    #[exclude(
        r"[/\\](?:feMorphology[/\\]huge-radius|radialGradient[/\\]focal-point-correction)\.svg$"
    )]
    input: PathBuf,
) -> Result<(), String> {
    if input.ends_with(Path::new("structure/svg/negative-size.svg"))
        || input.ends_with(Path::new("structure/svg/zero-size.svg"))
    {
        return input_contract::assert_invalid_size(&input);
    }
    if input.ends_with(Path::new("structure/svg/not-UTF-8-encoding.svg")) {
        return input_contract::assert_not_utf8(&input);
    }
    if input.ends_with(Path::new("structure/svg/no-size.svg")) {
        return input_contract::assert_no_size_contract(&input);
    }
    if browser_reference_tests::is_one_axis_contract(&input)? {
        return morphology_contract::assert_one_axis_contract(&input);
    }
    assert_reference(&input, Mode::Normal)
}

#[rstest]
fn upstream_extra_render_matches_reference(
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/extra/*.svg")] input: PathBuf,
) -> Result<(), String> {
    assert_reference(&input, extra_mode(&input)?)
}

fn extra_mode(input: &Path) -> Result<Mode<'static>, String> {
    let name = input
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or("invalid fixture name")?;
    let mode = match name {
        "filter-region-precision" => Mode::Extra(EXTRA_FILTER_SCALE),
        "filter-on-empty-group" | "filter-with-transform-on-shape" => Mode::Node("g1"),
        _ => Mode::Extra(1.0),
    };
    Ok(mode)
}

#[derive(Clone, Copy)]
enum Mode<'a> {
    Normal,
    Extra(f32),
    Node(&'a str),
}

fn assert_reference(input: &Path, mode: Mode<'_>) -> Result<(), String> {
    let (pixmap, differences) = compare_reference(input, mode)?;
    let reference_path = browser_reference_tests::reference_png_path(input)?;
    if differences > 0 {
        failure_artifact::save_if_requested(input, &reference_path, &pixmap, differences).map_err(
            |error| {
                format!(
                    "upstream fixture {}: {differences} different pixels; artifact: {error}",
                    input.display()
                )
            },
        )?;
    }
    assert_eq!(differences, 0, "upstream fixture {}", input.display());
    Ok(())
}

fn compare_reference(input: &Path, mode: Mode<'_>) -> Result<(Pixmap, usize), String> {
    let bytes =
        std::fs::read(input).map_err(|error| format!("read {}: {error}", input.display()))?;
    let tree = parse_source(input, &bytes)?;
    let pixmap = render(&tree, mode)?;
    let reference_path = browser_reference_tests::reference_png_path(input)?;
    let reference = std::fs::read(&reference_path)
        .map_err(|error| format!("read {}: {error}", reference_path.display()))?;
    let differences = compare::pixel_differences(&pixmap, &reference)?;
    Ok((pixmap, differences))
}

fn parse_source(input: &Path, bytes: &[u8]) -> Result<usvg::Tree, String> {
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        resources_dir: input.parent().map(Path::to_path_buf),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options)
        .map_err(|error| format!("parse {}: {error}", input.display()))?;
    Ok(tree)
}

fn render(tree: &usvg::Tree, mode: Mode<'_>) -> Result<Pixmap, String> {
    let size = match mode {
        Mode::Normal => tree.size().to_int_size().scale_to_width(NORMAL_WIDTH),
        Mode::Extra(scale) => tree.size().to_int_size().scale_by(scale),
        Mode::Node(id) => tree
            .node_by_id(id)
            .and_then(|node| node.abs_layer_bounding_box())
            .map(|bbox| bbox.size().to_int_size()),
    }
    .ok_or("upstream fixture render dimensions unavailable")?;
    let mut pixmap =
        Pixmap::new(size.width(), size.height()).ok_or("fixture pixmap allocation failed")?;
    render_into(tree, mode, &mut pixmap)?;
    Ok(pixmap)
}

fn render_into(tree: &usvg::Tree, mode: Mode<'_>, pixmap: &mut Pixmap) -> Result<(), String> {
    match mode {
        Mode::Normal => {
            let transform = Transform::from_scale(
                pixmap.width() as f32 / tree.size().width(),
                pixmap.height() as f32 / tree.size().height(),
            );
            resvg::render(tree, transform, &mut pixmap.as_mut());
        }
        Mode::Extra(scale) => {
            resvg::render(
                tree,
                Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
        }
        Mode::Node(id) => {
            let node = tree.node_by_id(id).ok_or("fixture node missing")?;
            resvg::render_node(node, Transform::identity(), &mut pixmap.as_mut())
                .ok_or("fixture node bounds missing")?;
        }
    }
    Ok(())
}
