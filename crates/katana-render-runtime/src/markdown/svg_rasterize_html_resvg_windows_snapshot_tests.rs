use super::snapshot_database;
use crate::markdown::svg_rasterize::font_file_fixture::{self, FONT_BYTES};
use crate::markdown::svg_rasterize::text_shaping;
use crate::markdown::svg_rasterize::{RasterizedSvg, SvgRasterizeOps, usvg};
use resvg::usvg::fontdb::{Database, Family, Source};
use std::sync::Arc;

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="32"><text x="4" y="24" font-family="Noto Sans" font-size="20">KRR</text></svg>"#;

#[test]
fn admitted_file_snapshot_preserves_face_metadata_and_generic_aliases()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let mut fixture = font_file_fixture::create(directory.path().join("font.ttf"), FONT_BYTES)?;
    let original = &mut fixture.database;
    original.set_sans_serif_family("Noto Sans");
    let expected = original
        .face(fixture.face_id)
        .ok_or("missing fixture face")?;
    let snapshot = text_shaping::with_validated_tree_parse(|| snapshot_database(original))
        .ok_or("admitted file did not produce a snapshot database")?;
    let actual = snapshot.faces().next().ok_or("snapshot face is missing")?;

    assert_eq!(actual.families, expected.families);
    assert_eq!(actual.post_script_name, expected.post_script_name);
    assert_eq!(actual.weight, expected.weight);
    assert_eq!(actual.stretch, expected.stretch);
    assert_eq!(actual.style, expected.style);
    assert!(matches!(&actual.source, Source::SharedFile(path, _) if path == &fixture.path));
    assert_eq!(
        snapshot.family_name(&Family::SansSerif),
        original.family_name(&Family::SansSerif)
    );
    Ok(())
}

#[test]
fn inactive_snapshot_scope_keeps_original_file_database() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let fixture = font_file_fixture::create(directory.path().join("font.ttf"), FONT_BYTES)?;
    assert!(snapshot_database(&fixture.database).is_none());
    Ok(())
}

#[test]
fn scoped_snapshot_keeps_file_source_pixels() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = font_file_fixture::create(directory.path().join("font.ttf"), FONT_BYTES)?;
    let expected = render_file_source(&fixture.database)?;
    let actual = render_snapshot_source(&fixture.database)?;

    assert!(expected.rgba.iter().any(|channel| *channel != 0));
    assert_eq!(
        (actual.width, actual.height, actual.rgba),
        (expected.width, expected.height, expected.rgba)
    );
    Ok(())
}

fn render_file_source(database: &Database) -> Result<RasterizedSvg, String> {
    let options = options_for(database);
    let tree = usvg::Tree::from_str(SVG, &options).map_err(|error| error.to_string())?;
    render_tree(&tree)
}

fn render_snapshot_source(database: &Database) -> Result<RasterizedSvg, String> {
    let tree = super::parse(SVG, options_for(database)).map_err(|error| error.to_string())?;
    render_tree(&tree)
}

fn options_for(database: &Database) -> usvg::Options<'static> {
    let mut options = usvg::Options {
        fontdb: Arc::new(database.clone()),
        ..usvg::Options::default()
    };
    text_shaping::install_html_font_resolver(&mut options);
    options
}

fn render_tree(tree: &usvg::Tree) -> Result<RasterizedSvg, String> {
    SvgRasterizeOps::render_html_tree_with_resvg_text(tree, 1.0, SVG, &Default::default())
        .map_err(|error| error.to_string())
}
