use super::{Database, checked_metrics, commit_metrics, font_for_face};
use resvg::usvg::fontdb::Source;
use std::{fs, io::Write};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const SIZE: f32 = 16.0;

#[test]
fn completed_line_metrics_are_discarded_if_the_file_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("font.ttf");
    fs::write(&path, FONT_BYTES)?;
    let mut database = Database::new();
    let id = database.load_font_source(Source::File(path.clone()));
    let [id] = id.as_slice() else {
        return Err("expected one face in the real font fixture".into());
    };
    let font = font_for_face(&database, *id).ok_or("font acquisition failed")?;
    let metrics = checked_metrics(&database, *id, &font, SIZE);
    assert!(metrics.is_some());
    assert_eq!(commit_metrics(&database, *id, &font, metrics), Ok(metrics));
    fs::OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(&[0])?;
    assert_eq!(commit_metrics(&database, *id, &font, metrics), Err(()));
    Ok(())
}
