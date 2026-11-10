use super::{resvg, usvg};
use rstest::rstest;
use std::path::{Path, PathBuf};
use tiny_skia::{Pixmap, Transform};

const VIEWPORT: u32 = 200;
const SAMPLE: (u32, u32) = (100, 50);
const RGBA_CHANNELS: usize = 4;
const CLIPPED_GREEN: [u8; RGBA_CHANNELS] = [0, 128, 0, 255];
const EMPTY_PIXEL: [u8; RGBA_CHANNELS] = [0, 0, 0, 0];
const FIXTURE_ROOT: &str =
    "../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/masking/clipPath";

#[rstest]
#[case::referenced_path("with-use-child.svg", CLIPPED_GREEN)]
#[case::invalid_group("with-invalid-child-via-use.svg", EMPTY_PIXEL)]
#[case::invalid_symbol("symbol-via-use-is-not-a-valid-child.svg", EMPTY_PIXEL)]
fn clip_children_accept_referenced_paths_and_reject_invalid_elements(
    #[case] name: &str,
    #[case] expected: [u8; RGBA_CHANNELS],
) -> Result<(), String> {
    let input = fixture_path(name);
    let tree = parse_fixture(&input)?;
    let pixel = render_sample(&tree)?;
    assert_eq!(pixel, expected, "fixture {name} at {SAMPLE:?}");
    Ok(())
}

#[rstest]
#[case::missing_target("#missing")]
#[case::invalid_reference("not-a-fragment")]
fn invalid_modern_use_href_does_not_fall_back_to_legacy_reference(
    #[case] modern: &str,
) -> Result<(), String> {
    let source = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="200" height="200"><defs><rect id="legacy" width="200" height="200" fill="red"/></defs><rect width="200" height="200" fill="blue"/><use xlink:href="#legacy" href="{modern}"/></svg>"##
    );
    let legacy_only = source.replace(&format!(" href=\"{modern}\""), "");
    assert_ne!(source, legacy_only);
    let actual = usvg::Tree::from_str(&source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let control = usvg::Tree::from_str(&legacy_only, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    assert_eq!(render_sample(&actual)?, [0, 0, 255, 255]);
    assert_eq!(render_sample(&control)?, [255, 0, 0, 255]);
    Ok(())
}

#[test]
fn empty_clip_on_symbol_use_does_not_hide_visible_sibling() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200"><defs><clipPath id="empty"/><symbol id="shape" viewBox="0 0 200 200"><rect width="200" height="200" fill="red"/></symbol></defs><rect width="200" height="200" fill="blue"/><use href="#shape" width="200" height="200" clip-path="url(#empty)"/></svg>"##;
    let tree = usvg::Tree::from_str(source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let unclipped = source.replace(" clip-path=\"url(#empty)\"", "");
    let control = usvg::Tree::from_str(&unclipped, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    assert_eq!(render_sample(&tree)?, [0, 0, 255, 255]);
    assert_eq!(render_sample(&control)?, [255, 0, 0, 255]);
    Ok(())
}

#[rstest]
#[case::percentage("50%", "75%")]
#[case::physical_inches("1in", "2in")]
fn symbol_use_width_resolves_units_before_clipping(
    #[case] narrow: &str,
    #[case] wide: &str,
) -> Result<(), String> {
    let source = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200"><defs><symbol id="shape" viewBox="0 0 10 10" preserveAspectRatio="none"><rect width="10" height="10" fill="red"/></symbol></defs><rect width="200" height="200" fill="blue"/><use href="#shape" width="{narrow}" height="100"/></svg>"##
    );
    let control = source.replace(&format!("width=\"{narrow}\""), &format!("width=\"{wide}\""));
    let actual = usvg::Tree::from_str(&source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let control = usvg::Tree::from_str(&control, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    assert_eq!(render_sample(&actual)?, [0, 0, 255, 255]);
    assert_eq!(render_sample(&control)?, [255, 0, 0, 255]);
    Ok(())
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_ROOT)
        .join(name)
}

fn parse_fixture(input: &Path) -> Result<usvg::Tree, String> {
    let bytes =
        std::fs::read(input).map_err(|error| format!("read {}: {error}", input.display()))?;
    let options = usvg::Options {
        resources_dir: input.parent().map(Path::to_path_buf),
        ..usvg::Options::default()
    };
    usvg::Tree::from_data(&bytes, &options)
        .map_err(|error| format!("parse {}: {error}", input.display()))
}

fn render_sample(tree: &usvg::Tree) -> Result<[u8; RGBA_CHANNELS], String> {
    let mut pixmap = Pixmap::new(VIEWPORT, VIEWPORT).ok_or("fixture pixmap allocation failed")?;
    resvg::render(tree, Transform::identity(), &mut pixmap.as_mut());
    let start = ((SAMPLE.1 * VIEWPORT + SAMPLE.0) as usize) * RGBA_CHANNELS;
    pixmap
        .data()
        .get(start..start + RGBA_CHANNELS)
        .ok_or("clip sample is outside the rendered image")?
        .try_into()
        .map_err(|_| "clip sample has an invalid channel count".to_owned())
}
