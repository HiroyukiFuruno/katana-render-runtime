use image::ImageReader;
use std::{fs, path::Path, process::Command};

use super::{fixture_path, sha, TempDir, BIN, HEIGHT, WIDTH};

struct FrameRecord {
    path: std::path::PathBuf,
    width: u32,
    height: u32,
    scale: f32,
    scroll_y: f32,
    generation: u64,
}

#[test]
fn candidate_cli_dispatches_native_capture_and_writes_png_frames(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::create()?;
    let fixture = fixture_path()?;
    let fixture_bytes = fs::read(&fixture)?;
    let output_dir = temp.0.join("native-frames");
    let output = Command::new(BIN)
        .arg("--input")
        .arg(fixture)
        .arg("--output")
        .arg(&output_dir)
        .arg("--width")
        .arg(WIDTH.to_string())
        .arg("--height")
        .arg(HEIGHT.to_string())
        .arg("--scale")
        .arg("1")
        .arg("--scroll-y")
        .arg("0")
        .arg("--scroll-y")
        .arg("600")
        .output()?;
    assert_capture_output(&output, &fixture_bytes, &output_dir)
}

fn assert_capture_output(
    output: &std::process::Output,
    fixture_bytes: &[u8],
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone())?;
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next(),
        Some(format!("source_sha256={}", sha(fixture_bytes)).as_str())
    );
    let records = lines
        .map(parse_frame_record)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(records.len(), 2);
    assert_ne!(records[0].path, records[1].path);
    assert_frame_records(&records, output_dir)
}

fn assert_frame_records(
    records: &[FrameRecord],
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut previous_generation = 0;
    for (record, expected_scroll_y) in records.iter().zip([0.0_f32, 600.0]) {
        assert_eq!((record.width, record.height), (WIDTH, HEIGHT));
        assert_eq!(record.scale, 1.0);
        assert_eq!(record.scroll_y, expected_scroll_y);
        assert!(record.generation > previous_generation);
        previous_generation = record.generation;
        assert!(record.path.starts_with(output_dir));
        let pixels = ImageReader::open(&record.path)?.decode()?.to_rgba8();
        assert_eq!(pixels.dimensions(), (WIDTH, HEIGHT));
        assert_eq!(pixels.as_raw().len(), WIDTH as usize * HEIGHT as usize * 4);
    }
    Ok(())
}

fn parse_frame_record(line: &str) -> Result<FrameRecord, Box<dyn std::error::Error>> {
    let (path, metadata) = line
        .strip_prefix("png=")
        .ok_or("frame record is missing its PNG path")?
        .split_once(" width=")
        .ok_or("frame record is missing its width")?;
    let fields = metadata.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err("frame record must contain five metadata fields".into());
    }
    Ok(FrameRecord {
        path: path.into(),
        width: fields[0].parse()?,
        height: frame_value(fields[1], "height")?.parse()?,
        scale: frame_value(fields[2], "scale")?.parse()?,
        scroll_y: frame_value(fields[3], "scroll_y")?.parse()?,
        generation: frame_value(fields[4], "generation")?.parse()?,
    })
}

fn frame_value<'a>(field: &'a str, expected: &str) -> Result<&'a str, Box<dyn std::error::Error>> {
    let (key, value) = field
        .split_once('=')
        .ok_or("invalid frame metadata field")?;
    if key != expected {
        return Err("frame metadata field order differs".into());
    }
    Ok(value)
}
