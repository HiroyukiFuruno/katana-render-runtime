use super::normalize_filter_shadow_colors;
use crate::markdown::svg_rasterize::SvgRasterizeOps;

const IMAGE_WIDTH: u32 = 64;
const IMAGE_HEIGHT: u32 = 48;
const RECT_X: u32 = 18;
const RECT_Y: u32 = 12;
const RECT_WIDTH: u32 = 16;
const RECT_HEIGHT: u32 = 16;
const SAMPLE_X: u32 = RECT_X + RECT_WIDTH / 2;
const SAMPLE_Y: u32 = RECT_Y + RECT_HEIGHT / 2;
const RGBA_CHANNELS: u32 = 4;
const OPAQUE_BLACK: &[u8] = &[0, 0, 0, 255];
const LEGACY_SHADOW: &str = "drop-shadow(1px 2px 2px rgba(185,185,185,.7))";
const MODERN_SHADOW: &str = "drop-shadow(1px 2px 2px rgba(185 185 185 / .7))";

#[test]
fn svg_css_shadow_preserves_valid_modern_rgba_and_renders_geometry() {
    /* WHY: 有効な Color 4 構文を旧色パーサーで誤変換せず、原文を次段へ渡す。 */
    let inline = format!("<rect style=\"filter:{MODERN_SHADOW}\"/>");
    let stylesheet = format!("<style>.unused{{filter:{MODERN_SHADOW}}}</style>");
    for markup in [inline, stylesheet] {
        assert_preserved_and_rendered(&natural_svg(&markup));
    }
}

#[test]
fn svg_css_shadow_preserves_valid_namespace_rule_and_renders_geometry() {
    /* WHY: 未対応の正常なセミコロン at-rule があれば、部分的な色変換を残さない。 */
    let markup = format!(
        "<style>@namespace svg url(http://www.w3.org/2000/svg); .unused{{filter:{LEGACY_SHADOW}}}</style>"
    );
    assert_preserved_and_rendered(&natural_svg(&markup));
}

#[test]
fn svg_css_shadow_preserves_valid_nested_rules_and_renders_geometry() {
    /* WHY: 親に一致しない未知の入れ子規則を保ち、子セレクターを全体へ漏らさない。 */
    let markup = format!("<style>.unused{{filter:{LEGACY_SHADOW}; .child{{opacity:.5}}}}</style>");
    let source = natural_svg(&markup);
    assert_preserved_and_rendered(&source);
    assert_public_pixels_match(&source, &natural_svg(""));
}

#[test]
fn svg_css_shadow_preserves_nested_variable_fallback_and_renders_geometry() {
    /* WHY: 既知変数が none に解決する値でも、未使用の複合 fallback を改変しない。 */
    let markup =
        format!("<rect style=\"--known:none;filter:var(--known, [{LEGACY_SHADOW}] {{(x)}})\"/>");
    assert_preserved_and_rendered(&natural_svg(&markup));
}

#[test]
fn svg_css_shadow_preserves_entity_and_cdata_source_ranges() {
    /* WHY: デコード後と元 XML の範囲が異なる値は、元の entity と CDATA を保持する。 */
    let entity_shadow = LEGACY_SHADOW.replace(',', "&#44;");
    let markup = [
        format!("<style>.unused{{filter:{entity_shadow}}}</style>"),
        format!("<style><![CDATA[.unused{{filter:{LEGACY_SHADOW}}}]]></style>"),
        format!("<rect style=\"filter:{entity_shadow}\"/>"),
    ];
    for fragment in markup {
        assert_preserved_and_rendered(&natural_svg(&fragment));
    }
}

fn natural_svg(markup: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{IMAGE_WIDTH}\" height=\"{IMAGE_HEIGHT}\">{markup}<rect x=\"{RECT_X}\" y=\"{RECT_Y}\" width=\"{RECT_WIDTH}\" height=\"{RECT_HEIGHT}\" class=\"child\" fill=\"black\"/></svg>"
    )
}

fn assert_preserved_and_rendered(source: &str) {
    let document = resvg::usvg::roxmltree::Document::parse(source);
    assert!(
        document.is_ok(),
        "正常 fixture XML: {:?}",
        document.as_ref().err()
    );
    assert!(
        normalize_filter_shadow_colors(source) == source,
        "SVG 原文が改変された"
    );
    assert!(
        SvgRasterizeOps::preprocess_for_rasterizer(source) == source,
        "公開前処理が SVG 原文を改変した"
    );
    let image = SvgRasterizeOps::rasterize_svg(source, 1.0);
    assert!(
        image.is_ok_and(|image| {
            assert_eq!((image.width, image.height), (IMAGE_WIDTH, IMAGE_HEIGHT));
            let offset = ((SAMPLE_Y * IMAGE_WIDTH + SAMPLE_X) * RGBA_CHANNELS) as usize;
            let end = offset + RGBA_CHANNELS as usize;
            assert_eq!(image.rgba.get(offset..end), Some(OPAQUE_BLACK));
            true
        }),
        "公開 SVG 描画失敗"
    );
}

fn assert_public_pixels_match(source: &str, reference: &str) {
    let candidate = SvgRasterizeOps::rasterize_svg(source, 1.0);
    let control = SvgRasterizeOps::rasterize_svg(reference, 1.0);
    assert!(
        candidate.is_ok_and(|candidate| {
            assert!(
                control.is_ok_and(|control| {
                    assert_eq!((control.width, control.height), (IMAGE_WIDTH, IMAGE_HEIGHT));
                    assert_eq!(
                        control.rgba.len(),
                        (IMAGE_WIDTH * IMAGE_HEIGHT * RGBA_CHANNELS) as usize
                    );
                    assert!(
                        candidate.rgba == control.rgba,
                        "未知の入れ子規則が既知矩形の画素を変更した"
                    );
                    true
                }),
                "独立対照 SVG 描画失敗"
            );
            true
        }),
        "入れ子 CSS SVG 描画失敗"
    );
}
