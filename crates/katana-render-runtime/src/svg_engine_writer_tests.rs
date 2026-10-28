/* Copyright 2023 the Resvg Authors
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use rstest::rstest;

use super::usvg;

#[path = "svg_engine_writer_fixture_inventory_tests.rs"]
mod fixture_inventory_tests;

const WRITER_FONT_DIRECTORY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts"
);
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
    let actual = serialize_upstream_writer_fixture(&input_path, case_name)?;
    let expected = std::fs::read_to_string(&expected_path).map_err(|error| {
        format!(
            "failed reading upstream writer expectation {}: {error}",
            expected_path.display()
        )
    })?;

    assert_eq!(actual, expected, "upstream writer case {case_name}");
    Ok(())
}

fn serialize_upstream_writer_fixture(input_path: &Path, case_name: &str) -> Result<String, String> {
    let input = std::fs::read_to_string(input_path).map_err(|error| {
        format!(
            "failed reading upstream writer input {}: {error}",
            input_path.display()
        )
    })?;
    let options = usvg::Options {
        fontdb: upstream_writer_font_database(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&input, &options)
        .map_err(|error| format!("failed parsing upstream writer fixture {case_name}: {error}"))?;
    Ok(tree.to_string(&writer_options(case_name)))
}

fn upstream_writer_font_database() -> Arc<usvg::fontdb::Database> {
    static DATABASE: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DATABASE
        .get_or_init(|| {
            let mut database = usvg::fontdb::Database::new();
            database.load_fonts_dir(WRITER_FONT_DIRECTORY);
            database.set_serif_family("Noto Serif");
            database.set_sans_serif_family("Noto Sans");
            database.set_cursive_family("Yellowtail");
            database.set_fantasy_family("Sedgwick Ave Display");
            database.set_monospace_family("Noto Mono");
            Arc::new(database)
        })
        .clone()
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
