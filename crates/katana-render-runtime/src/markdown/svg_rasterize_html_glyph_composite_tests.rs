use super::{HtmlGlyphRasterizer, Prepared, PreparedSpan, RGBA_CHANNELS, paint};
use resvg::usvg::{Node, Options, Tree, fontdb::Weight};
use skia_safe::{AlphaType, BlendMode, ColorType, Data, ImageInfo, Paint};
use tiny_skia::{Pixmap, Transform};

const WIDTH: u32 = 128;
const HEIGHT: u32 = 32;
const FAMILY: &str = "Helvetica";
const FRACTIONAL_ORIGIN: (f32, f32) = (8.296875, 24.609375);

struct Fixture {
    name: &'static str,
    text: &'static str,
    size: f32,
    weight: Weight,
    fill: &'static str,
    backdrop: [u8; RGBA_CHANNELS],
}

const LIGHT_BOLD: Fixture = Fixture {
    name: "light-bold-on-gray",
    text: " files",
    size: 13.99,
    weight: Weight::BOLD,
    fill: "#ffffff",
    backdrop: [153, 153, 153, 255],
};
const LIGHT_REGULAR: Fixture = Fixture {
    name: "light-regular-on-dark",
    text: "Reads and writes",
    size: 11.99,
    weight: Weight::NORMAL,
    fill: "#e0e0e0",
    backdrop: [30, 30, 30, 255],
};

#[test]
fn opaque_bold_glyph_composite_matches_direct_skia() -> Result<(), String> {
    compare_compositing(&LIGHT_BOLD)
}

#[test]
fn opaque_regular_glyph_composite_matches_direct_skia() -> Result<(), String> {
    compare_compositing(&LIGHT_REGULAR)
}

fn compare_compositing(fixture: &Fixture) -> Result<(), String> {
    let tree = fixture_tree(fixture)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("composite fixture text missing".into());
    };
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let prepared = prepared_fixture(&rasterizer, text)?;
    let mut direct = backdrop(fixture.backdrop)?;
    draw_on(&rasterizer, &prepared, &mut direct)?;
    let mut transparent = backdrop([0; RGBA_CHANNELS])?;
    draw_on(&rasterizer, &prepared, &mut transparent)?;
    let mut tiny = backdrop(fixture.backdrop)?;
    tiny.draw_pixmap(
        0,
        0,
        transparent.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    let image = skia_image_composite(&transparent, fixture.backdrop)?;
    let mut current = backdrop(fixture.backdrop)?;
    rasterizer.render_prepared(prepared, text, Transform::identity(), &mut current.as_mut())?;
    assert_compositing(fixture, &current, &direct, &tiny, &image)
}

fn prepared_fixture(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &resvg::usvg::Text,
) -> Result<Vec<PreparedSpan>, String> {
    let Prepared::Ready(prepared) = rasterizer.prepare(text)? else {
        return Err("composite fixture fell back from Skia".into());
    };
    if prepared.is_empty() || prepared.iter().any(|span| span.glyph_ids.is_empty()) {
        return Err("composite fixture produced no glyphs".into());
    }
    Ok(prepared)
}

fn fixture_tree(fixture: &Fixture) -> Result<Tree, String> {
    let database = super::super::font::html_font_db_for_text(FAMILY, fixture.text);
    let (x, y) = FRACTIONAL_ORIGIN;
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xml:space="preserve" width="{WIDTH}" height="{HEIGHT}"><text id="run" x="{x}" y="{y}" font-family="{FAMILY}" font-size="{}" font-weight="{}" fill="{}">{}</text></svg>"##,
        fixture.size, fixture.weight.0, fixture.fill, fixture.text,
    );
    Tree::from_str(
        &svg,
        &Options {
            fontdb: database,
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

fn backdrop(color: [u8; RGBA_CHANNELS]) -> Result<Pixmap, String> {
    let mut pixels = Pixmap::new(WIDTH, HEIGHT).ok_or("composite surface allocation failed")?;
    for pixel in pixels.data_mut().chunks_exact_mut(RGBA_CHANNELS) {
        pixel.copy_from_slice(&color);
    }
    Ok(pixels)
}

fn image_info() -> ImageInfo {
    ImageInfo::new(
        (WIDTH as i32, HEIGHT as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    )
}

fn draw_on(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    prepared: &[PreparedSpan],
    pixels: &mut Pixmap,
) -> Result<(), String> {
    let mut pixmap = pixels.as_mut();
    let mut target = super::wrap_pixmap(&mut pixmap)?;
    for span in prepared {
        let face = rasterizer
            .ensure_face(span.font)?
            .ok_or("composite fixture face missing")?;
        paint::draw_span(target.canvas(), span, face.typeface.as_skia(), 1.0)?;
    }
    Ok(())
}

fn skia_image_composite(source: &Pixmap, color: [u8; RGBA_CHANNELS]) -> Result<Pixmap, String> {
    let info = image_info();
    let image = skia_safe::images::raster_from_data(
        &info,
        Data::new_copy(source.data()),
        info.min_row_bytes(),
    )
    .ok_or("transparent glyph image unavailable")?;
    let mut pixels = backdrop(color)?;
    let mut pixmap = pixels.as_mut();
    let mut target = super::wrap_pixmap(&mut pixmap)?;
    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::SrcOver);
    target.canvas().draw_image(image, (0, 0), Some(&paint));
    drop(target);
    Ok(pixels)
}

fn assert_compositing(
    fixture: &Fixture,
    current: &Pixmap,
    direct: &Pixmap,
    tiny: &Pixmap,
    image: &Pixmap,
) -> Result<(), String> {
    let deltas = [
        ("current/direct", pixel_delta(current, direct)),
        ("current/tiny", pixel_delta(current, tiny)),
        ("tiny/skia-image", pixel_delta(tiny, image)),
        ("direct/skia-image", pixel_delta(direct, image)),
    ];
    if current.data() != direct.data() || tiny.data() != image.data() {
        return Err(format!(
            "{} differing pixels/max channel: {deltas:?}",
            fixture.name,
        ));
    }
    Ok(())
}

fn pixel_delta(left: &Pixmap, right: &Pixmap) -> (usize, u8) {
    left.data()
        .chunks_exact(RGBA_CHANNELS)
        .zip(right.data().chunks_exact(RGBA_CHANNELS))
        .fold((0, 0), |(count, maximum), (a, b)| {
            let delta = a
                .iter()
                .zip(b)
                .fold(0, |maximum, (a, b)| maximum.max(a.abs_diff(*b)));
            (count + usize::from(delta != 0), maximum.max(delta))
        })
}
