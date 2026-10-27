use super::{FRACTIONAL_BACKDROP, FRACTIONAL_SIZE, render_html_frame};
use skia_safe::{
    AlphaType, BlendMode, Color, ColorType, ImageInfo, M44, Matrix, Paint, PaintStyle, PathBuilder,
    Point, surfaces,
};
use tiny_skia::Pixmap;

const AA_RECT_ORIGIN: f32 = 4.296875;
const AA_RECT_EXTENT: f32 = 32.0;

#[test]
fn fractional_aa_only_matches_independent_skia_src_over_control() -> Result<(), String> {
    let svg = fractional_aa_only_svg();
    let actual = render_html_frame(&svg)?;
    let expected = render_src_over_control()?;
    assert_eq!(actual.as_slice(), expected.data());
    Ok(())
}

fn fractional_aa_only_svg() -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{FRACTIONAL_SIZE}" height="{FRACTIONAL_SIZE}"><rect x="{AA_RECT_ORIGIN}" y="{AA_RECT_ORIGIN}" width="{AA_RECT_EXTENT}" height="{AA_RECT_EXTENT}" fill="#1e1e1e"/></svg>"##
    )
}

fn render_src_over_control() -> Result<Pixmap, String> {
    let mut pixmap = uniform_backdrop()?;
    let path = fractional_rect_path();
    let paint = src_over_fill_paint();
    let dimensions = (FRACTIONAL_SIZE as i32, FRACTIONAL_SIZE as i32);
    let info = ImageInfo::new(dimensions, ColorType::RGBA8888, AlphaType::Premul, None);
    let identity = Matrix::new_identity();
    {
        let mut surface =
            surfaces::wrap_pixels(&info, pixmap.data_mut(), info.min_row_bytes(), None)
                .ok_or_else(|| "Skia AA control surface creation failed".to_owned())?;
        surface.canvas().set_matrix(&M44::from(&identity));
        surface.canvas().draw_path(&path, &paint);
    }
    Ok(pixmap)
}

fn fractional_rect_path() -> skia_safe::Path {
    let end = AA_RECT_ORIGIN + AA_RECT_EXTENT;
    let mut builder = PathBuilder::new();
    builder
        .move_to(Point::new(AA_RECT_ORIGIN, AA_RECT_ORIGIN))
        .line_to(Point::new(end, AA_RECT_ORIGIN))
        .line_to(Point::new(end, end))
        .line_to(Point::new(AA_RECT_ORIGIN, end))
        .close();
    builder.detach()
}

fn src_over_fill_paint() -> Paint {
    let mut paint = Paint::default();
    paint
        .set_color(Color::from_rgb(
            FRACTIONAL_BACKDROP[0],
            FRACTIONAL_BACKDROP[1],
            FRACTIONAL_BACKDROP[2],
        ))
        .set_style(PaintStyle::Fill)
        .set_anti_alias(true)
        .set_blend_mode(BlendMode::SrcOver);
    paint
}

fn uniform_backdrop() -> Result<Pixmap, String> {
    let Some(mut pixmap) = Pixmap::new(FRACTIONAL_SIZE, FRACTIONAL_SIZE) else {
        return Err("Skia AA control backdrop allocation failed".to_owned());
    };
    for pixel in pixmap
        .data_mut()
        .chunks_exact_mut(FRACTIONAL_BACKDROP.len())
    {
        pixel.copy_from_slice(&FRACTIONAL_BACKDROP);
    }
    Ok(pixmap)
}
