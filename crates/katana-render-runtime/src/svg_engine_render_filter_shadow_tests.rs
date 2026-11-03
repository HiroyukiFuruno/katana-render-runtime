use super::{Mode, parse_source, render, usvg};
use rstest::rstest;
use std::path::Path;

const SHADOW_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><defs><filter id="f" filterUnits="userSpaceOnUse" x="0" y="0" width="24" height="16" color-interpolation-filters="sRGB"><feDropShadow dx="8" dy="0" stdDeviation="SIGMA" flood-color="red"/></filter></defs><rect x="4" y="4" width="6" height="8" fill="black" filter="url(#f)"/></svg>"##;
const SOURCE_SAMPLE_X: u32 = 5;
const SHADOW_SAMPLE_X: u32 = 15;
const SAMPLE_Y: u32 = 8;

#[rstest]
#[case::small("0.5")]
#[case::large_below_box_threshold("1.5")]
fn small_shadow_blur_preserves_source_and_softens_offset_shadow(
    #[case] sigma: &str,
) -> Result<(), String> {
    let source = SHADOW_SVG.replace("SIGMA", sigma);
    let tree = parse_source(Path::new("shadow.svg"), source.as_bytes())?;
    let primitive = tree
        .filters()
        .first()
        .and_then(|filter| filter.primitives().first())
        .ok_or("drop shadow primitive missing")?;
    assert!(matches!(
        primitive.kind(),
        usvg::filter::Kind::DropShadow(_)
    ));
    let blurred = render(&tree, Mode::Extra(1.0))?;
    let sharp_source = SHADOW_SVG.replace("SIGMA", "0");
    let sharp_tree = parse_source(Path::new("shadow.svg"), sharp_source.as_bytes())?;
    let sharp = render(&sharp_tree, Mode::Extra(1.0))?;
    assert_shadow_pixels(&blurred)?;
    assert_ne!(blurred.data(), sharp.data(), "blur must change the shadow");
    Ok(())
}

fn assert_shadow_pixels(blurred: &tiny_skia::Pixmap) -> Result<(), String> {
    let source_pixel = blurred
        .pixel(SOURCE_SAMPLE_X, SAMPLE_Y)
        .ok_or("source pixel missing")?;
    let shadow_pixel = blurred
        .pixel(SHADOW_SAMPLE_X, SAMPLE_Y)
        .ok_or("shadow pixel missing")?;
    let background = blurred.pixel(0, 0).ok_or("background pixel missing")?;
    assert_eq!(source_pixel.alpha(), 255);
    assert_eq!(source_pixel.red(), 0);
    assert!(shadow_pixel.red() > 0 && shadow_pixel.alpha() > 0);
    assert_eq!(background.alpha(), 0);
    Ok(())
}
