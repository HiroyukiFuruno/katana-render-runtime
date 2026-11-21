use super::{Mode, render, upstream_writer_font_database, usvg};
use rstest::rstest;
use std::path::{Path, PathBuf};
use tiny_skia::Pixmap;

const CONTEXT_BLUE_RGB: Option<(u8, u8, u8)> = Some((0, 0, 255));
const CONTEXT_GREEN_RGB: Option<(u8, u8, u8)> = Some((0, 128, 0));
const RGBA_CHANNEL_COUNT: usize = 4;
#[derive(Clone, Copy)]
struct PixelSample {
    x: u32,
    y: u32,
    expected: [u8; RGBA_CHANNEL_COUNT],
}

const fn pixel_sample(x: u32, y: u32, expected: [u8; RGBA_CHANNEL_COUNT]) -> PixelSample {
    PixelSample { x, y, expected }
}

/* WHY: 元図の(100,10)はmarker内部かつ元path外で、1.5倍画像の(150,15)に対応する。 */
const DIRECT_MARKER_SAMPLE: PixelSample = pixel_sample(150, 15, [0, 0, 255, 255]);
const NESTED_USE_MARKER_SAMPLE: PixelSample = pixel_sample(150, 15, [0, 0, 255, 255]);
/* WHY: 終点175、refY10、1.5倍表示でviewport端がy277.5になる半被覆画素。 */
const ZERO_BBOX_SAMPLE: PixelSample = pixel_sample(150, 277, [0, 64, 0, 128]);

#[rstest]
fn upstream_context_cases_match_rendering_contract(
    #[files(
        "../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/painting/context/**/*.svg"
    )]
    input: PathBuf,
) -> Result<(), String> {
    /* WHY: 暗黙marker clipの既存hard-edge仕様に従い、境界の文脈色とclip契約を検証する。 */
    match input.file_name().and_then(|name| name.to_str()) {
        Some("in-marker.svg") => assert_context_marker(
            &input,
            CONTEXT_BLUE_RGB,
            CONTEXT_GREEN_RGB,
            DIRECT_MARKER_SAMPLE,
        ),
        Some("in-nested-use-and-marker.svg") => assert_context_marker(
            &input,
            CONTEXT_BLUE_RGB,
            CONTEXT_GREEN_RGB,
            NESTED_USE_MARKER_SAMPLE,
        ),
        Some("on-shape-with-zero-size-bbox.svg") => {
            assert_context_marker(&input, CONTEXT_GREEN_RGB, None, ZERO_BBOX_SAMPLE)
        }
        _ => super::assert_reference(&input, Mode::Normal),
    }
}

fn assert_context_marker(
    input: &Path,
    fill: Option<(u8, u8, u8)>,
    stroke: Option<(u8, u8, u8)>,
    sample: PixelSample,
) -> Result<(), String> {
    let tree = parse_fixture(input)?;
    let mut paths = Vec::new();
    collect_marker_paths(tree.root(), &mut paths);
    assert!(
        !paths.is_empty(),
        "marker child path missing: {}",
        input.display()
    );
    assert_marker_clip_is_hard_edge(&tree)?;
    assert_marker_paints(&paths, fill, stroke, input)?;
    let image = render(&tree, Mode::Normal)?;
    assert_marker_paths_rendered(&image, &tree, &paths);
    assert_eq!(
        premultiplied_pixel(&image, sample.x, sample.y)?,
        sample.expected
    );
    Ok(())
}

fn parse_fixture(input: &Path) -> Result<usvg::Tree, String> {
    let bytes = std::fs::read(input).map_err(|error| error.to_string())?;
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        resources_dir: input.parent().map(Path::to_path_buf),
        ..usvg::Options::default()
    };
    usvg::Tree::from_data(&bytes, &options).map_err(|error| error.to_string())
}

fn collect_marker_paths<'a>(group: &'a usvg::Group, paths: &mut Vec<&'a usvg::Path>) {
    for node in group.children() {
        if let usvg::Node::Group(child) = node {
            if child.clip_path().is_some_and(is_hard_edge_clip) {
                collect_group_paths(child, paths);
            } else {
                collect_marker_paths(child, paths);
            }
        }
    }
}

fn collect_group_paths<'a>(group: &'a usvg::Group, paths: &mut Vec<&'a usvg::Path>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(child) => collect_group_paths(child, paths),
            usvg::Node::Path(path) => paths.push(path),
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
    }
}

fn is_hard_edge_clip(clip: &usvg::ClipPath) -> bool {
    let mut modes = Vec::new();
    collect_clip_modes(clip.root(), &mut modes);
    !modes.is_empty()
        && modes
            .iter()
            .all(|mode| *mode == usvg::ShapeRendering::CrispEdges)
}

fn assert_marker_clip_is_hard_edge(tree: &usvg::Tree) -> Result<(), String> {
    let clips = tree.clip_paths();
    assert!(!clips.is_empty(), "marker viewport clip missing");
    for clip in clips {
        assert!(
            is_hard_edge_clip(clip),
            "implicit marker viewport clip must use hard edges"
        );
    }
    Ok(())
}

fn collect_clip_modes(group: &usvg::Group, modes: &mut Vec<usvg::ShapeRendering>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_clip_modes(group, modes),
            usvg::Node::Path(path) => modes.push(path.rendering_mode()),
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
    }
}

fn assert_marker_paints(
    paths: &[&usvg::Path],
    fill: Option<(u8, u8, u8)>,
    stroke: Option<(u8, u8, u8)>,
    input: &Path,
) -> Result<(), String> {
    for path in paths {
        assert_eq!(
            path.rendering_mode(),
            usvg::ShapeRendering::GeometricPrecision
        );
        assert_eq!(paint_rgb(path.fill().map(usvg::Fill::paint)), fill);
        assert_eq!(
            paint_rgb(path.stroke().map(usvg::Stroke::paint)),
            stroke,
            "context stroke mismatch: {}",
            input.display()
        );
    }
    Ok(())
}

fn paint_rgb(paint: Option<&usvg::Paint>) -> Option<(u8, u8, u8)> {
    match paint {
        Some(usvg::Paint::Color(color)) => Some((color.red, color.green, color.blue)),
        _ => None,
    }
}

fn assert_marker_paths_rendered(image: &Pixmap, tree: &usvg::Tree, paths: &[&usvg::Path]) {
    let scale_x = image.width() as f32 / tree.size().width();
    let scale_y = image.height() as f32 / tree.size().height();
    for path in paths {
        let bounds = path.abs_bounding_box();
        let left = (bounds.x() * scale_x).floor().max(0.0) as u32;
        let top = (bounds.y() * scale_y).floor().max(0.0) as u32;
        let right = (bounds.right() * scale_x).ceil().min(image.width() as f32) as u32;
        let bottom = (bounds.bottom() * scale_y)
            .ceil()
            .min(image.height() as f32) as u32;
        assert!(
            (top..bottom).any(|y| {
                (left..right).any(|x| image.pixel(x, y).is_some_and(|p| p.alpha() > 0))
            })
        );
    }
}

fn premultiplied_pixel(image: &Pixmap, x: u32, y: u32) -> Result<[u8; RGBA_CHANNEL_COUNT], String> {
    let pixel = image
        .pixel(x, y)
        .ok_or("zero-size marker sample outside image")?;
    Ok([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()])
}
