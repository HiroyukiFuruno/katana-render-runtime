use super::{VIEWPORT, usvg};
use flate2::{Compression, write::GzEncoder};
use std::io::Write;
use tiny_skia::{Pixmap, Transform};

#[test]
fn gzip_and_plain_svg_render_identical_pixels() -> Result<(), String> {
    let plain = br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <rect width="8" height="8" fill="blue"/></svg>"#;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(plain)
        .map_err(|error| error.to_string())?;
    let gzip = encoder.finish().map_err(|error| error.to_string())?;

    let expected = render_svg_data(plain)?;
    let actual = render_svg_data(&gzip)?;
    let first = expected.pixel(0, 0).ok_or("expected a rendered pixel")?;
    assert_eq!(
        (first.red(), first.green(), first.blue(), first.alpha()),
        (0, 0, 255, 255)
    );
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

fn render_svg_data(input: &[u8]) -> Result<Pixmap, String> {
    let tree = usvg::Tree::from_data(input, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    assert_eq!(tree.size().to_int_size().width(), VIEWPORT);
    assert_eq!(tree.size().to_int_size().height(), VIEWPORT);
    let mut pixmap = Pixmap::new(VIEWPORT, VIEWPORT).ok_or("pixmap allocation failed")?;
    super::super::resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    Ok(pixmap)
}
