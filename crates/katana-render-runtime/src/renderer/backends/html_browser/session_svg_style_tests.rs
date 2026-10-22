use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::{HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};

const WIDTH: u32 = 100;
const HEIGHT: u32 = 80;
const RGBA_CHANNELS: usize = 4;
const TRIANGLE_INTERIOR_X: usize = 75;
const TRIANGLE_INTERIOR_Y: usize = 40;
const SHADOW_X: usize = 42;
const SHADOW_Y: usize = 42;
const WHITE: [u8; RGBA_CHANNELS] = [255, 255, 255, 255];
const MARKER_SUFFIX_SELECTOR: &str = r##"#fixture [id$="-tip"]"##;
const MARKER_ID_SELECTOR: &str = "#fixture #head-tip";
const MARKER_TEMPLATE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" id="fixture" width="100" height="80" viewBox="0 0 100 80"><style>#fixture .marker{fill:#aaa;stroke:#aaa}__SELECTOR__{fill:transparent!important;stroke:#aaa!important;stroke-width:2}</style><defs><marker id="head-tip" class="marker" markerWidth="10" markerHeight="10" refX="0" refY="5" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0L10 5L0 10Z"/></marker></defs><rect width="100" height="80" fill="white"/><path d="M10 40H70" stroke="#aaa" stroke-width="2" marker-end="url(#head-tip)"/></svg>"##;
const SHADOW_TEMPLATE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80" viewBox="0 0 100 80"><style>.box{fill:#888;filter:drop-shadow(1px 2px 2px rgba(185,185,185,1))}__UNUSED__</style><rect width="100" height="80" fill="white"/><rect class="box" x="20" y="20" width="20" height="20"/></svg>"##;
const UNUSED_QUOTED_RULE: &str = r##"[data-label="unused"]{fill:red}"##;
const MULTIPLY_INTERIOR_X: usize = 30;
const MULTIPLY_INTERIOR_Y: usize = 30;
const INTERNAL_MULTIPLY_FILL: [u8; RGBA_CHANNELS] = [28, 28, 28, 255];
const MULTIPLY_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80" viewBox="0 0 100 80"><rect width="100" height="80" fill="#202020"/><g style="mix-blend-mode:multiply"><rect x="20" y="20" width="20" height="20" fill="#e0e0e0"/></g></svg>"##;

#[test]
fn inline_svg_quoted_marker_selector_keeps_triangle_interior_empty() {
    let suffix = MARKER_TEMPLATE.replace("__SELECTOR__", MARKER_SUFFIX_SELECTOR);
    let explicit = MARKER_TEMPLATE.replace("__SELECTOR__", MARKER_ID_SELECTOR);
    with_native_pixels(&explicit, |native| {
        with_native_pixels(&suffix, |suffix| {
            assert!(suffix == native, "native marker selector pixels differ");
        });
        assert_eq!(
            pixel_at(&native, TRIANGLE_INTERIOR_X, TRIANGLE_INTERIOR_Y),
            Some(WHITE.as_slice())
        );
        with_inline_pixels(&suffix, |inline_suffix| {
            with_inline_pixels(&explicit, |inline_explicit| {
                assert!(
                    inline_suffix == inline_explicit,
                    "inline marker selector pixels differ"
                );
            });
            assert!(
                inline_suffix == native,
                "inline/native marker pixels differ"
            );
        });
    });
}

#[test]
fn inline_svg_unused_quoted_css_keeps_shadow_pixels_unchanged() {
    let plain = SHADOW_TEMPLATE.replace("__UNUSED__", "");
    let quoted = SHADOW_TEMPLATE.replace("__UNUSED__", UNUSED_QUOTED_RULE);
    with_native_pixels(&plain, |native| {
        with_native_pixels(&quoted, |quoted| {
            assert!(quoted == native, "native quoted shadow pixels differ");
        });
        let shadow = pixel_at(&native, SHADOW_X, SHADOW_Y);
        assert!(shadow.is_some(), "shadow control pixel missing");
        assert_ne!(shadow, Some(WHITE.as_slice()));
        with_inline_pixels(&plain, |inline_plain| {
            with_inline_pixels(&quoted, |inline_quoted| {
                assert!(
                    inline_plain == inline_quoted,
                    "inline quoted shadow pixels differ"
                );
                assert!(
                    inline_quoted == native,
                    "inline/native shadow pixels differ"
                );
            });
        });
    });
}

#[test]
fn inline_svg_multiply_preserves_its_explicit_internal_backdrop() {
    with_native_pixels(MULTIPLY_SVG, |native| {
        assert_eq!(
            pixel_at(&native, MULTIPLY_INTERIOR_X, MULTIPLY_INTERIOR_Y),
            Some(INTERNAL_MULTIPLY_FILL.as_slice())
        );
        with_inline_pixels(MULTIPLY_SVG, |actual| {
            assert_eq!(
                pixel_at(&actual, MULTIPLY_INTERIOR_X, MULTIPLY_INTERIOR_Y),
                Some(INTERNAL_MULTIPLY_FILL.as_slice()),
                "inline SVG must preserve multiplication against its internal background"
            );
        });
    });
}

fn with_native_pixels(svg: &str, action: impl FnOnce(Vec<u8>)) {
    with_success(
        SvgRasterizeOps::rasterize_svg(svg, 1.0),
        "native SVG",
        |raster| {
            assert_eq!((raster.width, raster.height), (WIDTH, HEIGHT));
            assert_eq!(
                raster.rgba.len(),
                WIDTH as usize * HEIGHT as usize * RGBA_CHANNELS
            );
            action(raster.rgba);
        },
    );
}

fn with_inline_pixels(svg: &str, action: impl FnOnce(Vec<u8>)) {
    let html = format!(
        "<!doctype html><html><head><style>html,body{{margin:0;width:{WIDTH}px;height:{HEIGHT}px}}</style></head><body>{svg}</body></html>"
    );
    let source = HtmlBrowserSource::new(html, "https://example.test/quoted-svg-css.html");
    with_success(source, "HTML source", |source| {
        let viewport = HtmlBrowserViewport::new(WIDTH, HEIGHT, 1.0);
        with_success(viewport, "viewport", |viewport| {
            with_success(
                HtmlRuntime.open(source, viewport),
                "HTML session",
                |session| {
                    with_closed_session_pixels(session, action);
                },
            );
        });
    });
}

fn with_closed_session_pixels(mut session: HtmlBrowserSession, action: impl FnOnce(Vec<u8>)) {
    let frame = session.latest_frame().map(|frame| {
        (
            frame.viewport.width,
            frame.viewport.height,
            frame.pixels.clone(),
        )
    });
    let close = session.close();
    assert!(
        close.is_ok(),
        "HTML session close: {:?}",
        close.as_ref().err()
    );
    assert!(frame.is_some(), "inline SVG frame missing");
    if let Some((width, height, pixels)) = frame {
        assert_eq!((width, height), (WIDTH, HEIGHT));
        assert_eq!(
            pixels.len(),
            WIDTH as usize * HEIGHT as usize * RGBA_CHANNELS
        );
        action(pixels);
    }
}

fn with_success<T, E>(result: Result<T, E>, context: &str, action: impl FnOnce(T)) {
    assert!(
        result.is_ok_and(|value| {
            action(value);
            true
        }),
        "{context}"
    );
}

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> Option<&[u8]> {
    let offset = (y * WIDTH as usize + x) * RGBA_CHANNELS;
    pixels.get(offset..offset + RGBA_CHANNELS)
}
