use super::{FAMILY, SAMPLE, emit_stage};
use resvg::usvg::fontdb::{Database, Source, Weight};
use serde_json::json;
use sha2::{Digest, Sha256};
use skia_safe::{AlphaType, Color, ColorType, Font, ImageInfo, Paint, surfaces};
use std::path::PathBuf;
use std::time::Instant;

const PIXEL_WIDTH: i32 = 128;
const PIXEL_HEIGHT: i32 = 64;
const PIXEL_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const SAMPLE_FONT_SIZE: f32 = 32.0;
const SAMPLE_ORIGIN_X: i32 = 4;
const SAMPLE_BASELINE_Y: i32 = 42;

pub(super) struct PhysicalFont {
    pub(super) path: PathBuf,
    pub(super) index: u32,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct GlyphDrawObservation {
    pub(super) elapsed_ns: u128,
    pub(super) glyph_ids: Vec<u16>,
    pub(super) pixel_sha256: String,
}

struct GlyphRenderObservation {
    elapsed_ns: u128,
    glyph_ids: Vec<u16>,
}

pub(super) fn selected_physical_font(
    database: &Database,
) -> Result<PhysicalFont, Box<dyn std::error::Error>> {
    let face = database
        .faces()
        .find(|face| {
            face.weight == Weight::NORMAL && face.families.iter().any(|(name, _)| name == FAMILY)
        })
        .ok_or("selected family face missing from HTML font database")?;
    let path = match &face.source {
        Source::File(path) | Source::SharedFile(path, _) => path.clone(),
        Source::Binary(_) => return Err("selected face is not file-backed".into()),
    };
    let (bytes, index) = database
        .with_face_data(face.id, |data, index| (data.to_vec(), index))
        .ok_or("selected physical face bytes unavailable")?;
    Ok(PhysicalFont { path, index, bytes })
}

pub(super) fn emit_source_stage(source: &PhysicalFont, elapsed_ns: u128) {
    emit_stage(
        "selected-physical-font-bytes",
        elapsed_ns,
        json!({
            "family": FAMILY,
            "weight": Weight::NORMAL.0,
            "path": source.path.display().to_string(),
            "face_index": source.index,
            "byte_length": source.bytes.len(),
            "sha256": sha256(&source.bytes),
        }),
    );
}

pub(super) fn assert_typeface_source(
    typeface: &skia_safe::Typeface,
    source: &PhysicalFont,
) -> Result<(), Box<dyn std::error::Error>> {
    let (data, index) = typeface
        .to_existing_font_data()
        .ok_or("loaded typeface did not expose its physical source")?;
    if index != source.index || data.as_bytes() != source.bytes.as_slice() {
        return Err("loaded typeface differs from the selected physical source".into());
    }
    Ok(())
}

pub(super) fn draw_first_glyph(
    typeface: &skia_safe::Typeface,
) -> Result<GlyphDrawObservation, Box<dyn std::error::Error>> {
    let mut pixels = vec![0; PIXEL_WIDTH as usize * PIXEL_HEIGHT as usize * PIXEL_CHANNELS];
    let rendered = render_glyph(typeface, &mut pixels)?;
    if !pixels
        .chunks_exact(PIXEL_CHANNELS)
        .any(|pixel| pixel[ALPHA_CHANNEL] != 0)
    {
        return Err("first glyph draw produced an empty pixel buffer".into());
    }
    Ok(GlyphDrawObservation {
        elapsed_ns: rendered.elapsed_ns,
        glyph_ids: rendered.glyph_ids,
        pixel_sha256: sha256(&pixels),
    })
}

fn render_glyph(
    typeface: &skia_safe::Typeface,
    pixels: &mut [u8],
) -> Result<GlyphRenderObservation, Box<dyn std::error::Error>> {
    let info = ImageInfo::new(
        (PIXEL_WIDTH, PIXEL_HEIGHT),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut surface = surfaces::wrap_pixels(&info, pixels, info.min_row_bytes(), None)
        .ok_or("Skia pixel surface allocation failed")?;
    let font = Font::new(typeface.clone(), SAMPLE_FONT_SIZE);
    let glyph_ids = font.text_to_glyphs_vec(SAMPLE);
    if glyph_ids.is_empty() || glyph_ids.contains(&0) {
        return Err("selected physical face did not resolve the sample glyphs".into());
    }
    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    let started = Instant::now();
    surface
        .canvas()
        .draw_str(SAMPLE, (SAMPLE_ORIGIN_X, SAMPLE_BASELINE_Y), &font, &paint);
    Ok(GlyphRenderObservation {
        elapsed_ns: started.elapsed().as_nanos(),
        glyph_ids,
    })
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
