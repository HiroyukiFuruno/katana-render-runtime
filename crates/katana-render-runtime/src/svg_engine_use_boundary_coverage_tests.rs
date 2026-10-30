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
