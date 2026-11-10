/* Copyright 2023 the Resvg Authors
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use rstest::rstest;

use super::{resvg, usvg};
use tiny_skia::{Pixmap, Transform};

#[path = "svg_engine_writer_property_tests.rs"]
mod writer_property_tests;

#[path = "svg_engine_writer_coverage_tests.rs"]
mod writer_coverage_tests;

#[path = "svg_engine_writer_font_metrics_tests.rs"]
mod writer_font_metrics_tests;

#[path = "svg_engine_writer_corpus_tests.rs"]
mod writer_corpus_tests;

#[path = "svg_engine_writer_radial_tests.rs"]
mod radial_tests;

#[path = "svg_engine_colr_fixture_tests.rs"]
mod colr_fixture_tests;

#[path = "svg_engine_writer_displacement_tests.rs"]
mod displacement_tests;
#[path = "svg_engine_writer_filter_regression_tests.rs"]
mod filter_regression_tests;
#[path = "svg_engine_writer_fixture_inventory_tests.rs"]
mod fixture_inventory_tests;
#[path = "svg_engine_writer_image_roundtrip_tests.rs"]
mod image_roundtrip_tests;
#[path = "svg_engine_writer_text_style_coverage_tests.rs"]
mod text_style_coverage_tests;

#[rstest]
#[case::linear("<text x='10' y='35' font-family='Noto Sans' font-size='18' ROTATE>Text</text>")]
#[case::path(
    "<defs><path id='curve' d='M10 35 H110'/></defs><text font-family='Noto Sans' font-size='18' ROTATE><textPath href='#curve'>Text</textPath></text>"
)]
fn explicit_zero_rotation_preserves_text_pixels(#[case] body: &str) -> Result<(), String> {
    let wrap = |attribute: &str| {
        format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='120' height='60'>{}</svg>",
            body.replace("ROTATE", attribute)
        )
    };
    let rotated = parse_writer_svg(&wrap("rotate='0'"), "zero-rotation")?;
    let reference = parse_writer_svg(&wrap(""), "implicit-rotation")?;
    let rendered =
        writer_coverage_tests::assert_pixels_equal(&rotated, &reference, "zero rotation")?;
    assert!(
        rendered
            .data()
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel[ALPHA_CHANNEL] != 0)
    );
    Ok(())
}

#[test]
fn explicit_default_text_styles_are_omitted_from_preserved_svg() -> Result<(), String> {
    let implicit = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><text x="50" y="50" font-family="Noto Sans" font-size="18">Edge Text</text></svg>"#;
    let explicit = implicit.replace(
        "<text ",
        r#"<text style="font-stretch:normal;dominant-baseline:auto;alignment-baseline:auto" "#,
    );
    let options = usvg::WriteOptions {
        preserve_text: true,
        ..usvg::WriteOptions::default()
    };
    let serialized = parse_writer_svg(&explicit, "explicit defaults")?.to_string(&options);
    let default_svg = parse_writer_svg(implicit, "implicit defaults")?.to_string(&options);
    assert_eq!(serialized, default_svg);
    for attribute in ["font-stretch=", "dominant-baseline=", "alignment-baseline="] {
        assert!(
            !serialized.contains(attribute),
            "writer emitted {attribute}"
        );
    }
    Ok(())
}

fn first_text(group: &usvg::Group) -> Option<&usvg::Text> {
    group.children().iter().find_map(|node| match node {
        usvg::Node::Text(text) => Some(text.as_ref()),
        usvg::Node::Group(group) => first_text(group),
        _ => None,
    })
}

const WRITER_FONT_DIRECTORY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts"
);
const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const PIXEL_DIAGNOSTIC_PREVIEW_LIMIT: usize = 16;
const PRESERVE_TEXT_CASES: [&str; 9] = [
    "preserve-text-multiple-font-families",
    "preserve-text-on-path",
    "preserve-text-in-clip-path",
    "preserve-text-in-mask",
    "preserve-text-in-pattern",
    "preserve-text-with-dx-and-dy",
    "preserve-text-with-rotate",
    "preserve-text-with-complex-text-decoration",
    "preserve-text-with-nested-baseline-shift",
];

#[rstest]
fn upstream_usvg_writer_matches_reference(
    #[files("../../tests/fixtures/svg-engine/upstream/usvg/tests/files/*.svg")]
    #[exclude("-expected\\.svg$")]
    input_path: PathBuf,
) -> Result<(), String> {
    let case_name = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            format!(
                "upstream writer fixture filename must be UTF-8: {}",
                input_path.display()
            )
        })?;
    let expected_path = input_path.with_file_name(format!("{case_name}-expected.svg"));
    let tree = parse_upstream_writer_fixture(&input_path, case_name)?;
    let actual = tree.to_string(&writer_options(case_name));
    let expected = std::fs::read_to_string(&expected_path).map_err(|error| {
        format!(
            "failed reading upstream writer expectation {}: {error}",
            expected_path.display()
        )
    })?;

    assert_eq!(actual, expected, "upstream writer case {case_name}");
    assert_render_determinism(&tree, &actual, &expected, case_name)?;
    Ok(())
}

fn parse_upstream_writer_fixture(input_path: &Path, case_name: &str) -> Result<usvg::Tree, String> {
    let input = std::fs::read_to_string(input_path).map_err(|error| {
        format!(
            "failed reading upstream writer input {}: {error}",
            input_path.display()
        )
    })?;
    parse_writer_svg(&input, case_name)
}

fn parse_writer_svg(input: &str, case_name: &str) -> Result<usvg::Tree, String> {
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(input, &options)
        .map_err(|error| format!("failed parsing upstream writer fixture {case_name}: {error}"))
}

fn assert_render_determinism(
    tree: &usvg::Tree,
    actual: &str,
    expected: &str,
    case_name: &str,
) -> Result<(), String> {
    let parsed_actual = parse_writer_svg(actual, case_name)?;
    let parsed_expected = parse_writer_svg(expected, case_name)?;
    assert_eq!(
        tree.size(),
        parsed_actual.size(),
        "viewport for {case_name}"
    );
    assert_eq!(
        parsed_actual.size(),
        parsed_expected.size(),
        "reference viewport for {case_name}"
    );
    let original = render_tree(tree)?;
    let actual_image = render_tree(&parsed_actual)?;
    let expected_image = render_tree(&parsed_expected)?;
    assert_filter_visible(case_name, &original);
    assert_filter_visible(case_name, &actual_image);
    assert!(
        actual_image.data() == expected_image.data(),
        "pixels for {case_name}"
    );
    Ok(())
}

fn assert_filter_visible(case_name: &str, original: &Pixmap) {
    if case_name == "filter-with-object-units-multi-use" {
        assert!(
            original
                .data()
                .chunks_exact(RGBA_CHANNELS)
                .any(|pixel| pixel[ALPHA_CHANNEL] != 0),
            "filter control is transparent"
        );
    }
}

fn render_tree(tree: &usvg::Tree) -> Result<Pixmap, String> {
    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height())
        .ok_or("writer fixture pixmap allocation failed")?;
    resvg::render(tree, Transform::identity(), &mut pixmap.as_mut());
    Ok(pixmap)
}

fn assert_rgba_pixel(
    pixmap: &Pixmap,
    point: (u32, u32),
    expected: (u8, u8, u8, u8),
) -> Result<(), String> {
    let actual = pixmap
        .pixel(point.0, point.1)
        .map(|pixel| (pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()));
    if actual != Some(expected) {
        assert_eq!(
            actual,
            Some(expected),
            "pixel ({},{}) in {}x{} RGBA image; first {} pixels: [{}]",
            point.0,
            point.1,
            pixmap.width(),
            pixmap.height(),
            PIXEL_DIAGNOSTIC_PREVIEW_LIMIT,
            pixel_preview(pixmap)
        );
    }
    Ok(())
}

fn pixel_preview(pixmap: &Pixmap) -> String {
    pixmap
        .data()
        .chunks_exact(RGBA_CHANNELS)
        .take(PIXEL_DIAGNOSTIC_PREVIEW_LIMIT)
        .enumerate()
        .map(|(index, rgba)| {
            let x = index as u32 % pixmap.width();
            let y = index as u32 / pixmap.width();
            format!("({x},{y})={:?}", [rgba[0], rgba[1], rgba[2], rgba[3]])
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn upstream_writer_font_database() -> Result<Arc<usvg::fontdb::Database>, String> {
    static DATABASE: OnceLock<Result<Arc<usvg::fontdb::Database>, String>> = OnceLock::new();
    DATABASE.get_or_init(load_upstream_fonts).clone()
}

fn load_upstream_fonts() -> Result<Arc<usvg::fontdb::Database>, String> {
    let entries = std::fs::read_dir(WRITER_FONT_DIRECTORY).map_err(|error| error.to_string())?;
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut database = usvg::fontdb::Database::new();
    for path in paths {
        if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("ttf" | "ttc" | "otf" | "otc" | "TTF" | "TTC" | "OTF" | "OTC")
        ) && let Err(error) = database.load_font_file(&path)
        {
            log::warn!("Failed to load '{}' cause {}.", path.display(), error);
        }
    }
    database.set_serif_family("Noto Serif");
    database.set_sans_serif_family("Noto Sans");
    database.set_cursive_family("Yellowtail");
    database.set_fantasy_family("Sedgwick Ave Display");
    database.set_monospace_family("Noto Mono");
    Ok(Arc::new(database))
}

fn writer_options(case_name: &str) -> usvg::WriteOptions {
    usvg::WriteOptions {
        id_prefix: (case_name == "filter-id-with-prefix").then(|| "prefix-".to_owned()),
        preserve_text: PRESERVE_TEXT_CASES.contains(&case_name),
        coordinates_precision: 4,
        transforms_precision: 4,
        ..usvg::WriteOptions::default()
    }
}
