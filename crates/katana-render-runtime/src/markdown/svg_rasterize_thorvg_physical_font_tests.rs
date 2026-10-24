use resvg::usvg::fontdb::{Database, Family, Query, Style, Weight};
use sha2::{Digest, Sha256};
use thorvg::Thorvg;

use crate::markdown::svg_rasterize::font::html_font_db_for_text;
use crate::markdown::svg_rasterize::render::RasterTarget;

const FAMILY: &str = "Trebuchet MS";
const WIDTH: u32 = 160;
const HEIGHT: u32 = 64;
const SCALE: f32 = 1.0;
const OUTPUT_COUNT: usize = 4;
const CHANNEL_COUNT: usize = 4;
const ALPHA_CHANNEL: usize = 3;

#[test]
fn physical_regular_and_bold_aliases_are_rendered_separately() -> Result<(), String> {
    let database = html_font_db_for_text(FAMILY, "Aa");
    let regular = physical_face(&database, Weight::NORMAL)?;
    let bold = physical_face(&database, Weight::BOLD)?;
    let engine = Thorvg::init(0).map_err(|error| format!("engine init failed: {error:?}"))?;
    register(&engine, "KrrPhysicalRegular", &regular)?;
    register(&engine, "KrrPhysicalBold", &bold)?;
    let target = RasterTarget::new(canvas_size()?, SCALE);
    let outputs = render_aliases(&engine, &target)?;
    assert_outputs(&outputs)?;
    eprintln!(
        "thorvg physical font probe regular={:?} index={} bold={:?} index={} pixels={} deltas={:?}",
        regular.sha256,
        regular.index,
        bold.sha256,
        bold.index,
        outputs.len(),
        pairwise_deltas(&outputs)
    );
    Ok(())
}

struct PhysicalFace {
    bytes: Vec<u8>,
    index: u32,
    sha256: Vec<u8>,
}

fn physical_face(database: &Database, weight: Weight) -> Result<PhysicalFace, String> {
    let id = database
        .query(&Query {
            families: &[Family::Name(FAMILY)],
            weight,
            style: Style::Normal,
            ..Query::default()
        })
        .ok_or_else(|| format!("Trebuchet MS weight {} missing", weight.0))?;
    let face = database
        .face(id)
        .ok_or("Trebuchet MS face metadata missing")?;
    if face.weight != weight || face.style != Style::Normal {
        return Err("Trebuchet MS selected face style or weight differs".to_string());
    }
    let (bytes, index) = database
        .with_face_data(id, |bytes, index| (bytes.to_vec(), index))
        .ok_or("Trebuchet MS physical bytes unavailable")?;
    if index != 0 {
        return Err(format!("Trebuchet MS TTC index {index} is unsupported"));
    }
    let sha256 = Sha256::digest(&bytes).to_vec();
    Ok(PhysicalFace {
        bytes,
        index,
        sha256,
    })
}

fn register(engine: &Thorvg, alias: &str, face: &PhysicalFace) -> Result<(), String> {
    engine
        .load_font_data(alias, &face.bytes, Some("ttf"))
        .map_err(|error| format!("font registration {alias} failed: {error:?}"))
}

fn canvas_size() -> Result<resvg::usvg::Size, String> {
    resvg::usvg::Size::from_wh(WIDTH as f32, HEIGHT as f32)
        .ok_or_else(|| "invalid physical font probe size".to_string())
}

fn render_aliases(engine: &Thorvg, target: &RasterTarget) -> Result<Vec<Vec<u8>>, String> {
    let cases = [
        ("KrrPhysicalRegular", Weight::NORMAL.0),
        ("KrrPhysicalRegular", Weight::BOLD.0),
        ("KrrPhysicalBold", Weight::NORMAL.0),
        ("KrrPhysicalBold", Weight::BOLD.0),
    ];
    cases
        .into_iter()
        .map(|(alias, weight)| {
            let svg = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{WIDTH}\" height=\"{HEIGHT}\"><text x=\"4\" y=\"40\" font-family=\"{alias}\" font-size=\"28\" font-weight=\"{weight}\">Aa</text></svg>"
            );
            super::render_one(
                engine,
                &svg,
                canvas_size()?,
                target,
            )
        })
        .collect()
}

fn assert_outputs(outputs: &[Vec<u8>]) -> Result<(), String> {
    if outputs.len() != OUTPUT_COUNT || outputs.iter().any(|pixels| pixels.is_empty()) {
        return Err("physical font probe did not produce four nonempty outputs".to_string());
    }
    let expected = WIDTH as usize * HEIGHT as usize * CHANNEL_COUNT;
    if outputs.iter().any(|pixels| pixels.len() != expected) {
        return Err("physical font probe canvas dimensions differ".to_string());
    }
    for pixels in outputs {
        assert_premultiplied_ink(pixels)?;
    }
    Ok(())
}

fn assert_premultiplied_ink(pixels: &[u8]) -> Result<(), String> {
    let mut has_ink = false;
    for pixel in pixels.chunks_exact(CHANNEL_COUNT) {
        let alpha = pixel[ALPHA_CHANNEL];
        has_ink |= alpha > 0;
        if pixel[..ALPHA_CHANNEL]
            .iter()
            .any(|channel| *channel > alpha)
        {
            return Err(format!("non-premultiplied physical font pixel: {pixel:?}"));
        }
    }
    if !has_ink {
        return Err("physical font output contains no alpha ink".to_string());
    }
    Ok(())
}

fn pairwise_deltas(outputs: &[Vec<u8>]) -> Vec<usize> {
    outputs
        .iter()
        .enumerate()
        .flat_map(|(index, pixels)| {
            outputs[index + 1..]
                .iter()
                .map(move |other| pixels.iter().zip(other).filter(|(a, b)| a != b).count())
        })
        .collect()
}
