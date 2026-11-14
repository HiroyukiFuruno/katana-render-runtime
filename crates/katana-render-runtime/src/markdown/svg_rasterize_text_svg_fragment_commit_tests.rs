use super::{
    FontShape, commit_fragment_run, measure_svg_run, ordered_scalar_advances, svg_font_instance,
};
use resvg::usvg::fontdb::{Database, Source};
use std::{fs, io::Write};

const FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const TEXT: &str = "AV";
const FONT_SIZE: f32 = 16.0;
const FONT_WEIGHT: u16 = 400;

#[test]
fn stale_font_cannot_publish_a_previously_shaped_svg_fragment()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("font.ttf");
    fs::write(&path, FONT)?;
    let mut database = Database::new();
    let faces = database.load_font_source(Source::File(path.clone()));
    let id = *faces.first().ok_or("fixture font missing")?;
    let shape = FontShape::new(FONT_SIZE, FONT_WEIGHT, false);
    let (font, instance) = svg_font_instance(&database, id, shape)?;
    let run = measure_svg_run(&database, id, TEXT, shape, &instance)?;
    let advances = ordered_scalar_advances(TEXT, &run.glyphs, &instance, FONT_SIZE);
    let accepted = commit_fragment_run(&database, id, &font, run.metrics, advances.clone())?;
    assert_eq!(
        accepted.ok_or("expected the original run")?.1.len(),
        TEXT.len()
    );

    fs::OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(&[0])?;
    let rejected = commit_fragment_run(&database, id, &font, run.metrics, advances);
    assert_eq!(
        rejected.err().as_deref(),
        Some("SVG font source changed during shaping")
    );
    Ok(())
}
