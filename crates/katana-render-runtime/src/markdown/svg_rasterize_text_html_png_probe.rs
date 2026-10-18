use image::ImageEncoder;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;
const RGBA_CHANNEL_COUNT: usize = 4;
const SVG_SHA: &str = "8e4a91dd90ea7f9ebb0607aa1ff592ed9d548def31691bb79aed5c63701dfc12";

#[test]
#[ignore = "diagnostic only; verified public SVG and fresh TMP PNG paths required"]
fn records_html_raster_route_for_verified_native_svg() -> ProbeResult<()> {
    let input = required_path("KRR_HTML_RASTER_PROBE_SVG_PATH")?;
    let output = required_path("KRR_HTML_RASTER_PROBE_PNG_PATH")?;
    let bytes = std::fs::read(&input)?;
    assert_eq!(sha256(&bytes), SVG_SHA, "unexpected native SVG input");
    let svg = std::str::from_utf8(&bytes)?;
    let raster = super::super::SvgRasterizeOps::rasterize_html_svg(svg, 1.0)?;
    let png = encode_png(&raster)?;
    write_fresh(&output, &png)?;
    eprintln!(
        "html-raster-route svg_sha256={} png_sha256={} width={} height={} output={}",
        SVG_SHA,
        sha256(&png),
        raster.width,
        raster.height,
        output.display()
    );
    Ok(())
}

#[test]
fn html_raster_contract_encodes_a_self_contained_svg_to_fresh_png() -> ProbeResult<()> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12"><rect width="8" height="8" fill="black"/></svg>"#;
    let raster = super::super::SvgRasterizeOps::rasterize_html_svg(svg, 1.0)?;
    let png = encode_png(&raster)?;
    let directory = super::probe_test_support::TempProbeDirectory::create()?;
    let output = directory.file("raster.png");
    write_fresh(&output, &png)?;
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let decoded = image::load_from_memory(&std::fs::read(output)?)?;
    assert_eq!(
        (decoded.width(), decoded.height()),
        (raster.width, raster.height)
    );
    assert!(
        raster
            .rgba
            .chunks_exact(RGBA_CHANNEL_COUNT)
            .any(|pixel| pixel[3] > 0)
    );
    assert_eq!(sha256(&png).len(), 64);
    Ok(())
}

fn required_path(name: &str) -> ProbeResult<PathBuf> {
    let path = std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or("probe path missing")?;
    if !path.is_absolute() {
        return Err("absolute probe path required".into());
    }
    Ok(path)
}

fn encode_png(raster: &super::super::RasterizedSvg) -> ProbeResult<Vec<u8>> {
    let expected = usize::try_from(raster.width)
        .ok()
        .and_then(|width| {
            usize::try_from(raster.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(RGBA_CHANNEL_COUNT))
        .ok_or("RGBA dimensions overflow")?;
    assert_eq!(raster.rgba.len(), expected, "RGBA size mismatch");
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        &raster.rgba,
        raster.width,
        raster.height,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(png)
}

pub(super) use super::probe_test_support::write_fresh;

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
