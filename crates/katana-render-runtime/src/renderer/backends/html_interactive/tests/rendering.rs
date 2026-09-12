use super::support::{TestResult, frame_contains_rgb, start, start_with_viewport, to_string};
use crate::renderer::backends::html_browser::{
    HTML_BROWSER_MAX_SOURCE_BYTES, HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserSource,
    HtmlBrowserViewport,
};
use crate::renderer::backends::html_document::HtmlDocumentNode;

const TEST_VIEWPORT_WIDTH: u32 = 320;
const TEST_VIEWPORT_HEIGHT: u32 = 240;
const STRUCTURED_CASCADE_DOCUMENT: &str = r#"<style>
:root { --accent: #123456; }
body { margin: 0; }
.shell { display: flex; gap: 8px; }
.card { width: 80px; height: 40px; background: var(--accent); }
.card[data-tone="priority"] { width: 120px; background: #ef4444 !important; }
#priority { background: #3b82f6; }
@media screen and (min-width: 300px) {
  .shell { display: grid; grid-template-columns: 80px 120px; }
}
</style>
<main class=shell>
  <section class=card>Variable</section>
  <section id=priority class=card data-tone=priority>Important</section>
</main>"#;
const BOX_MODEL_DOCUMENT: &str = r#"<style>
html, body { margin: 0; }
.content-box { box-sizing: content-box; width: 100px; height: 20px; padding: 10px; border: 2px solid #123456; background: #abcdef; }
.border-box { box-sizing: border-box; width: 100px; height: 40px; padding: 10px; border: 2px solid #654321; border-radius: 6px; overflow: hidden; background: #fedcba; font-family: Inter, sans-serif; font-style: italic; text-align: center; letter-spacing: 2px; }
.overflowing { height: 100px; background: #ef4444; }
</style>
<div class=content-box></div>
<div class=border-box>Hi<div class=overflowing>Clipped</div></div>"#;
const FIXED_SLIDESHOW_NAVIGATION_DOCUMENT: &str = r#"<style>
html, body { margin: 0; }
.nav-bar {
  position: fixed; left: 0; right: 0; bottom: 0; height: 60px;
  display: flex; align-items: center; justify-content: space-between;
  padding: 0 28px; font-family: Arial, sans-serif;
}
.nav-left, .nav-right { display: flex; align-items: center; gap: 12px; }
.nav-bar button {
  box-sizing: border-box; width: 40px; height: 40px;
  display: flex; align-items: center; justify-content: center;
}
.page-indicator { font-size: 15px; letter-spacing: 0.02em; }
.page-indicator b { font-weight: 700; }
.appendix-tag { display: none; }
.hint { font-size: 13px; }
</style>
<div class="nav-bar">
  <div class="nav-left">
    <button>‹</button><button>›</button>
    <span class="page-indicator"><b>4</b> / <span>14</span></span>
    <span class="appendix-tag">APPENDIX</span>
  </div>
  <div class="nav-right"><span class="hint">← → でページ送り ／ Home・End で先頭・末尾 ／ 図はクリックで拡大</span></div>
</div>"#;
const FLEX_AUTO_MIN_HEIGHT_DOCUMENT: &str = r#"<style>
body { margin: 0; }
h1 { margin: 0; padding: 0; }
.column { display: flex; flex-direction: column; width: 200px; height: 120px; }
.heading { font-size: 32px; line-height: 40px; margin-bottom: 20px; background: #2457d6; }
.content { flex: 1; min-height: 0; background: #ef4444; }
</style>
<main class="column"><h1 class="heading">Title</h1><div class="content"></div></main>"#;
const SLIDE_HEADING_DOCUMENT: &str = r#"<style>
html, body { margin: 0; }
.slide { display: flex; flex-direction: column; width: 100vw; padding: 44px 76px 80px; box-sizing: border-box; }
h1 { margin: 0; padding: 0; }
.title {
  font-family: "Noto Sans";
  font-size: 42.842px;
  font-weight: 700;
  line-height: 55.6946px;
  letter-spacing: 0.42842px;
  font-feature-settings: "palt" 1;
}
</style>
<main class="slide"><h1 class="title">LibreChat fork to MCP Hub to Code Sandbox in three layers architecture</h1></main>"#;

#[test]
fn renders_css_and_hides_document_metadata() -> TestResult {
    let session = start(metadata_document())?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert_eq!(frame.pixels.len(), 320 * 240 * 4);
    assert!(
        frame
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [255, 255, 255, 255])
    );
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains("Visible title"));
    assert!(!snapshot.contains("Hidden"));
    Ok(())
}

#[test]
fn document_lifecycle_mutation_is_present_in_the_initial_browser_frame() -> TestResult {
    let session = start(
        r#"<p id=status style="background: #ef4444; padding: 12px">Waiting</p>
<script>
document.addEventListener('DOMContentLoaded', () => {
  const status = document.getElementById('status');
  status.textContent = `Ready:${document.readyState}`;
  status.style.backgroundColor = '#35a853';
});
</script>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "initial lifecycle frame must exist".to_string())?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(snapshot.contains("Ready:interactive"), "{snapshot}");
    assert!(snapshot.contains("background-color: #35a853"), "{snapshot}");
    assert!(frame_contains_rgb(frame, [53, 168, 83]));
    assert!(!frame_contains_rgb(frame, [239, 68, 68]));
    Ok(())
}

#[test]
fn body_layout_rules_do_not_expand_descendant_boxes() -> TestResult {
    let session = start(
        r#"<style>body { min-height: 720px; background: #ef4444; color: #172554; }</style><p>Visible body text</p>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "body layout frame must exist".to_string())?;

    assert!(frame_contains_rgb(frame, [239, 68, 68]));
    assert!(session.content_height < 800.0, "{}", session.content_height);
    Ok(())
}

#[test]
fn pseudo_content_appearance_and_rotation_render_as_css_generated_boxes() -> TestResult {
    let mut session = start(pseudo_content_and_rotation_html())?;
    assert_pseudo_content_and_rotation_has_expected_frame(
        session
            .latest_frame()
            .ok_or_else(|| "generated content frame must exist".to_string())?,
    )?;
    assert_pseudo_content_and_rotation_layout(&mut session)?;
    Ok(())
}

fn pseudo_content_and_rotation_html() -> &'static str {
    r#"<style>
html, body { margin: 0; }
.host { position: relative; width: 100px; height: 50px; }
.host:after { content: ''; position: absolute; left: 0; right: 0; bottom: 0; height: 10px; background: #123456; }
.toggle { position: absolute; left: 0; top: 0; width: 60px; height: 34px; appearance: none; transform: rotate(90deg); }
.toggle:before { content: '❯'; color: #e6e6e6; font-size: 22px; padding: 6px; }
.toggle:checked:before { color: #737373; }
</style>
<main class=host><input id=toggle class=toggle type=checkbox checked></main>"#
}

fn assert_pseudo_content_and_rotation_has_expected_frame(frame: &HtmlBrowserFrame) -> TestResult {
    assert!(frame_contains_rgb(frame, [18, 52, 86]));
    Ok(())
}

fn assert_pseudo_content_and_rotation_layout(
    session: &mut super::super::HtmlInteractiveSession,
) -> TestResult {
    assert_target_exists(session, "toggle")?;
    let layout = session.layout().map_err(to_string)?;
    assert!(layout.svg.contains('❯'), "{}", layout.svg);
    assert!(
        layout.svg.contains(r#"transform="rotate(90 "#),
        "{}",
        layout.svg
    );
    assert!(layout.svg.contains(r##"fill="#737373""##), "{}", layout.svg);
    assert!(
        !layout.svg.contains(r##"stroke="#8c959f""##),
        "{}",
        layout.svg
    );
    let node_id = session
        .runtime
        .node_for_element_id("toggle")
        .ok_or_else(|| "toggle node is missing".to_string())?
        .0;
    let element = layout
        .element_boxes
        .iter()
        .find(|element| element.node_id == node_id)
        .ok_or_else(|| "toggle layout box is missing".to_string())?;
    assert!((element.rotation_degrees - 90.0).abs() < f32::EPSILON);
    Ok(())
}

#[test]
fn horizontal_auto_margins_center_a_max_width_block_in_the_viewport() -> TestResult {
    let mut session = start(
        r#"<style>html, body { margin: 0; } #centered { max-width: 100px; margin: 0 auto; height: 20px; background: #ef4444; }</style><main id=centered></main>"#,
    )?;
    let node_id = session
        .runtime
        .node_for_element_id("centered")
        .ok_or_else(|| "centered node is missing".to_string())?
        .0;
    let element = session
        .element_boxes
        .iter()
        .find(|element| element.node_id == node_id)
        .ok_or_else(|| "centered layout box is missing".to_string())?;

    assert!((element.x - 110.0).abs() < 0.01, "{element:?}");
    assert!((element.width - 100.0).abs() < 0.01, "{element:?}");
    Ok(())
}

#[test]
fn markerless_inline_list_items_share_one_inline_row() -> TestResult {
    let mut session = start(
        r#"<style>html, body { margin: 0; } ul { margin: 0; padding: 0; list-style: none; } li { display: inline; }</style><ul><li id=all>All</li><li id=active>Active</li><li id=completed>Completed</li></ul>"#,
    )?;
    let boxes = ["all", "active", "completed"]
        .map(|id| element_box_for_id(&mut session, id))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    assert!(boxes.windows(2).all(|pair| pair[0].x < pair[1].x));
    assert!(
        boxes
            .windows(2)
            .all(|pair| (pair[0].y - pair[1].y).abs() < 0.01)
    );
    Ok(())
}

#[test]
fn inline_anchor_filters_keep_intrinsic_width_and_single_line_height() -> TestResult {
    let mut session = start(
        r##"<style>
html, body { margin: 0; font: 14px/1.4 Arial, sans-serif; }
#host { position: relative; width: 550px; height: 40px; }
#filters { position: absolute; left: 0; right: 0; margin: 0; padding: 0; list-style: none; text-align: center; }
#filters li { display: inline; }
#filters a { margin: 3px; padding: 3px 7px; border: 1px solid transparent; text-decoration: none; }
</style>
<footer id=host><ul id=filters>
<li><a id=all href="#/">All</a></li>
<li><a id=active href="#/active">Active</a></li>
<li><a id=completed href="#/completed">Completed</a></li>
</ul></footer>"##,
    )?;
    let boxes = ["all", "active", "completed"]
        .map(|id| element_box_for_id(&mut session, id))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    assert_filter_boxes(&boxes);
    Ok(())
}

fn assert_filter_boxes(boxes: &[super::super::types::ElementBox]) {
    assert!(
        boxes.iter().all(|element| element.height < 30.0),
        "{boxes:?}"
    );
    assert!(
        boxes
            .windows(2)
            .all(|pair| (pair[0].y - pair[1].y).abs() < 0.01),
        "{boxes:?}"
    );
    assert!(
        boxes.windows(2).all(|pair| pair[0].x < pair[1].x),
        "{boxes:?}"
    );
}

#[test]
fn floats_leave_absolute_static_position_on_the_parent_content_line() -> TestResult {
    let mut session = start(
        r#"<style>
html, body { margin: 0; font: 14px/1.4 Arial, sans-serif; }
#host { position: relative; box-sizing: border-box; width: 300px; height: 41px; padding: 10px 15px; border-top: 1px solid #ddd; }
#count { float: left; }
#right { float: right; }
#filters { position: absolute; left: 0; right: 0; margin: 0; padding: 0; }
</style>
<footer id=host><span id=count>1 item left</span><ul id=filters></ul><span id=right>Clear</span></footer>"#,
    )?;
    let boxes = ["host", "count", "filters", "right"]
        .map(|id| element_box_for_id(&mut session, id))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    assert!((boxes[0].height - 41.0).abs() < 0.01, "{boxes:?}");
    assert!((boxes[1].y - boxes[2].y).abs() < 0.01, "{boxes:?}");
    assert!((boxes[3].y - boxes[2].y).abs() < 0.01, "{boxes:?}");
    assert!(boxes[3].x > boxes[1].x, "{boxes:?}");
    Ok(())
}

#[test]
fn empty_text_input_paints_its_placeholder() -> TestResult {
    let mut session = start(
        r#"<style>html, body { margin: 0; }</style><input placeholder="What needs to be done?">"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout.svg.contains("What needs to be done?"),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn script_cleared_input_value_replaces_the_host_edit_cache_with_its_placeholder() -> TestResult {
    let mut session = start(
        r#"<input id=field value=initial placeholder=Ready><script>document.getElementById('field').value='';</script>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(layout.svg.contains("Ready"), "{}", layout.svg);
    assert!(!layout.svg.contains(">initial<"), "{}", layout.svg);
    Ok(())
}

#[test]
fn inner_html_fragments_do_not_insert_viewport_height_wrappers() -> TestResult {
    let mut session = start(
        r#"<style>
html, body { margin: 0; }
ul { margin: 0; padding: 0; list-style: none; }
li { position: relative; }
li label { display: block; padding: 10px; }
</style>
<main><ul id=list></ul><footer id=footer>Footer</footer></main>
<script>document.getElementById('list').innerHTML = '<li id="item"><label>Task</label></li>';</script>"#,
    )?;
    let item = element_box_for_id(&mut session, "item")?;
    let footer = element_box_for_id(&mut session, "footer")?;

    assert!(item.height < 100.0, "{item:?}");
    assert!(footer.y < 100.0, "item={item:?} footer={footer:?}");
    Ok(())
}

#[test]
fn absolute_checkbox_with_vertical_auto_margins_centers_in_its_relative_container() -> TestResult {
    let mut session = start(
        r#"<style>html, body { margin: 0; } #host { position: relative; width: 100px; height: 60px; } #choice { position: absolute; top: 0; bottom: 0; width: 20px; height: auto; margin: auto 0; }</style><div id=host><input id=choice type=checkbox></div>"#,
    )?;
    let node_id = session
        .runtime
        .node_for_element_id("choice")
        .ok_or_else(|| "choice node is missing".to_string())?
        .0;
    let element = session
        .element_boxes
        .iter()
        .find(|element| element.node_id == node_id)
        .ok_or_else(|| "choice layout box is missing".to_string())?;

    assert!((element.y - 20.0).abs() < 0.01, "{element:?}");
    assert!((element.height - 20.0).abs() < 0.01, "{element:?}");
    Ok(())
}

#[test]
fn absolute_checkbox_centers_in_an_auto_height_relative_container() -> TestResult {
    let mut session = start(
        r#"<style>
html, body { margin: 0; }
#host { position: relative; width: 100px; }
#host label { display: block; padding: 20px 0; line-height: 20px; }
#choice { position: absolute; top: 0; bottom: 0; width: 20px; height: auto; margin: auto 0; }
</style><div id=host><input id=choice type=checkbox><label>Task</label></div>"#,
    )?;
    let host = element_box_for_id(&mut session, "host")?;
    let choice = element_box_for_id(&mut session, "choice")?;

    assert!((host.height - 60.0).abs() < 0.01, "{host:?}");
    assert!(
        (choice.y - (host.y + (host.height - choice.height) / 2.0)).abs() < 0.01,
        "host={host:?} choice={choice:?}"
    );
    Ok(())
}

#[test]
fn styled_summary_and_link_paint_their_clickable_boxes() -> TestResult {
    let mut session = start(styled_link_document())?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert!(frame_contains_rgb(frame, [219, 234, 254]));
    assert!(frame_contains_rgb(frame, [237, 233, 254]));
    assert_target_exists(&mut session, "more")?;
    assert_target_exists(&mut session, "next")
}

#[test]
fn embedded_svg_preserves_vector_geometry_in_the_browser_frame() -> TestResult {
    let session = start(
        r##"<svg width="120" height="80" viewBox="0 0 120 80" role="img">
        <rect x="10" y="10" width="100" height="60" rx="8" fill="#e11d48"/>
        <circle cx="60" cy="40" r="18" fill="#22c55e"/>
        </svg>"##,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "embedded SVG frame must exist".to_string())?;

    assert!(frame_contains_rgb(frame, [225, 29, 72]));
    assert!(frame_contains_rgb(frame, [34, 197, 94]));
    Ok(())
}

#[test]
fn secondary_heading_levels_paint_with_their_tag_metrics() -> TestResult {
    let session = start("<h2>Section</h2><h3>Subsection</h3>")?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "heading frame must exist".to_string())?;

    assert!(
        frame
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [255, 255, 255, 255])
    );
    Ok(())
}

#[test]
fn styled_details_paints_its_closed_summary_container() -> TestResult {
    let session = start(
        r#"<style>details { background: #c4e2ff; border: 1px solid #2563eb; padding: 8px; }</style><details><summary>More</summary><p>Hidden panel</p></details>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert!(frame_contains_rgb(frame, [196, 226, 255]));
    Ok(())
}

#[test]
fn standalone_summary_paints_without_becoming_a_details_toggle_target() -> TestResult {
    let session = start("<summary>Standalone summary</summary>")?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "summary frame must exist".to_string())?;

    assert!(
        frame
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [255, 255, 255, 255])
    );
    assert!(session.hit_targets.is_empty());
    Ok(())
}

#[test]
fn structural_html_is_laid_out_as_tables_lists_rules_and_wrapped_text() -> TestResult {
    let session = start(
        r#"<main style="padding: 6px; margin: 4px; width: 280px; min-height: 120px; border: 1px solid #334155">
plain text that must use the direct text rendering path rather than a label
<hr style="border: 1px solid #dc2626">
<ul><li>unordered first</li><li>unordered second</li></ul>
<ol><li>ordered first</li><li>ordered second</li></ol>
<table><thead><tr><th>Feature</th><th>Status</th></tr></thead><tbody><tr></tr><tr><td>Direct runtime frame</td><td>Ready</td></tr></tbody></table>
<p style="display: none">hidden body content</p>
</main>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "structural frame must exist".to_string())?;

    assert!(
        frame
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [255, 255, 255, 255])
    );
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains("Direct runtime frame"));
    Ok(())
}

#[test]
fn inline_style_applies_box_font_and_visibility_declarations() -> TestResult {
    let session = start(
        r#"<p style="color: #14532d; background: #bbf7d0; border: 2px dashed #166534; padding: 8px; margin-top: 4px; margin-bottom: 6px; width: 240px; height: 48px; min-height: 64px; font-size: 18px; line-height: 24px; font-weight: 700; text-decoration: underline; letter-spacing: 1px; malformed">Styled content</p>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "styled frame must exist".to_string())?;

    assert!(frame_contains_rgb(frame, [187, 247, 208]));
    assert!(frame_contains_rgb(frame, [20, 83, 45]));
    Ok(())
}

#[test]
fn compound_child_and_descendant_selectors_control_paint() -> TestResult {
    let session = start(
        r#"<style>
main > section.card[data-state="ready"] p.message.emphasis {
  background: #35a853;
  color: #17372a;
  padding: 4px;
}
</style>
<main>
  <section class="card" data-state="ready">
    <p class="message emphasis">Selector target</p>
  </section>
  <section class="card" data-state="pending">
    <p class="message emphasis">Non-target</p>
  </section>
</main>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "selector frame must exist".to_string())?;

    assert!(frame_contains_rgb(frame, [53, 168, 83]));
    Ok(())
}

#[test]
fn structured_cascade_applies_variables_important_grid_and_media_query() -> TestResult {
    let mut session = start(STRUCTURED_CASCADE_DOCUMENT)?;
    let layout = session.layout().map_err(to_string)?;
    assert_structured_cascade_svg(&layout.svg);
    Ok(())
}

#[test]
fn author_body_margin_and_background_own_the_complete_viewport() -> TestResult {
    let mut session = start(
        r#"<style>html, body { margin: 0; background: #123456; }</style><p>Full viewport</p>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="0" y="0" width="320" height="240" fill="#123456"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn typed_box_model_overflow_and_typography_reach_layout_and_paint() -> TestResult {
    let mut session = start(BOX_MODEL_DOCUMENT)?;
    let layout = session.layout().map_err(to_string)?;
    assert_box_model_svg(&layout.svg);
    Ok(())
}

fn assert_structured_cascade_svg(svg: &str) {
    assert!(
        svg.contains(r##"<rect x="0" y="0" width="80" height="40" fill="#123456"/>"##),
        "{svg}"
    );
    assert!(
        svg.contains(r##"<rect x="88" y="0" width="120" height="40" fill="#ef4444"/>"##),
        "{svg}"
    );
}

fn assert_box_model_svg(svg: &str) {
    assert!(svg.contains(r##"<rect x="0" y="0" width="124" height="44" fill="#abcdef"/>"##));
    assert!(svg.contains(
        r##"<rect x="0" y="44" width="100" height="40" rx="6" ry="6" fill="#fedcba"/>"##
    ));
    for expected in [
        r#"<clipPath id="krr-clip-0">"#,
        r#"font-family="Inter, sans-serif""#,
        r#"font-style="italic""#,
        r#"letter-spacing="2""#,
        r#"lengthAdjust="spacingAndGlyphs""#,
    ] {
        assert!(svg.contains(expected), "{svg}");
    }
}

#[test]
fn url_attribute_selector_controls_stylesheet_paint() -> TestResult {
    let session = start(
        r#"<style>
a[href="https://example.com"] { background: #35a853; padding: 4px; }
</style>
<a href="https://example.com">Matching URL</a>
<a href="https://example.net">Different URL</a>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "selector frame must exist".to_string())?;

    assert!(frame_contains_rgb(frame, [53, 168, 83]));
    Ok(())
}

#[test]
fn four_value_box_shorthands_control_painted_geometry() -> TestResult {
    let mut session = start(
        r#"<main style="background: #123456; margin: 2px 4px 6px 8px; padding: 10px 12px 14px 16px"><p>Box model</p></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="16" y="10" width="292" height="56" fill="#123456"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn more_specific_padding_longhand_wins_over_later_shorthand_during_paint() -> TestResult {
    let mut session = start(
        r#"<style>
.card { padding-left: 20px; }
div { padding: 0; }
</style>
<div class="card">Cascade text</div>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(layout.svg.contains(r#"<text x="28""#), "{}", layout.svg);
    Ok(())
}

#[test]
fn flex_flow_positions_items_with_css_gap() -> TestResult {
    let mut session = start(
        r#"<style>
main { display: flex; gap: 10px; width: 280px; }
.card { width: 100px; height: 40px; }
#first { background: #ef4444; }
#second { background: #3b82f6; }
</style>
<main><section id=first class=card>First</section><section id=second class=card>Second</section></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="8" y="8" width="100" height="40" fill="#ef4444"/>"##),
        "{}",
        layout.svg
    );
    assert!(
        layout
            .svg
            .contains(r##"<rect x="118" y="8" width="100" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn flex_flow_resolves_percentage_item_width_once() -> TestResult {
    let mut session = start(
        r#"<main style="display:flex; width:280px">
<section style="width:50%; height:40px; background:#ef4444">Half width</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="8" y="8" width="140" height="40" fill="#ef4444"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn flex_one_uses_zero_basis_and_distributes_equal_card_widths() -> TestResult {
    let mut session = start(
        r#"<main style="display:flex;width:280px;gap:8px;align-items:stretch">
<section style="box-sizing:border-box;flex:1;padding:8px;background:#ef4444">Short</section>
<span style="flex-shrink:0">→</span>
<section style="box-sizing:border-box;flex:1;padding:8px;background:#35a853">A much longer card label</section>
<span style="flex-shrink:0">→</span>
<section style="box-sizing:border-box;flex:1;padding:8px;background:#2457d6">Medium label</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;
    let (widths, heights) = colored_card_dimensions(&layout.svg)?;

    assert!(
        (widths[0] - widths[1]).abs() <= 1.0,
        "{:?}\n{}",
        widths,
        layout.svg
    );
    assert!(
        (widths[1] - widths[2]).abs() <= 1.0,
        "{:?}\n{}",
        widths,
        layout.svg
    );
    assert_eq!(heights, [heights[0]; 3], "{}", layout.svg);
    assert!(heights[0] > 60.0, "{:?}\n{}", heights, layout.svg);
    Ok(())
}

#[test]
fn flex_grow_distributes_content_space_after_each_items_box_edges() -> TestResult {
    let mut session = start_with_viewport(
        r#"<main style="display:flex;width:1078px;gap:22px">
<section style="flex:1;padding:20px 24px;border:1px solid #111;background:#ef4444">One</section>
<section style="flex:2;padding:20px 24px;border:1px solid #111;background:#35a853">Two</section>
</main>"#,
        1_200,
        240,
    )?;
    let layout = session.layout().map_err(to_string)?;
    let first = rect_width_for_fill(&layout.svg, "#ef4444")?;
    let second = rect_width_for_fill(&layout.svg, "#35a853")?;

    assert!(
        (first - 368.666_66).abs() <= 0.5,
        "{}\n{}",
        first,
        layout.svg
    );
    assert!(
        (second - 687.333_3).abs() <= 0.5,
        "{}\n{}",
        second,
        layout.svg
    );
    Ok(())
}

#[test]
fn flex_flow_paints_a_direct_text_node() -> TestResult {
    let mut session = start(r#"<main style="display:flex; width:280px">Direct flow text</main>"#)?;
    let layout = session.layout().map_err(to_string)?;

    assert!(layout.svg.contains("Direct flow text"), "{}", layout.svg);
    Ok(())
}

#[test]
fn hidden_flex_item_does_not_consume_width_or_gap() -> TestResult {
    let mut session = start(
        r#"<main style="display:flex; gap:10px; width:280px">
<section hidden style="width:100px; height:40px; background:#ef4444">Hidden</section>
<section style="width:100px; height:40px; background:#3b82f6">Visible</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="8" y="8" width="100" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    assert!(!layout.svg.contains("Hidden"), "{}", layout.svg);
    Ok(())
}

#[test]
fn hidden_grid_item_does_not_consume_a_track_or_gap() -> TestResult {
    let mut session = start(
        r#"<main style="display:grid; grid-template-columns:100px 100px; gap:10px; width:280px">
<section hidden style="height:40px; background:#ef4444">Hidden</section>
<section style="height:40px; background:#3b82f6">Visible</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="8" y="8" width="100" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    assert!(!layout.svg.contains("Hidden"), "{}", layout.svg);
    Ok(())
}

#[test]
fn hidden_grid_item_preserves_first_track_position_for_following_items() -> TestResult {
    let mut session = start(
        r#"<main style="display:grid; grid-template-columns:70px 150px; gap:10px; width:280px">
<section hidden style="height:40px; background:#ef4444">Hidden</section>
<section style="height:40px; background:#3b82f6">Visible</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="8" y="8" width="70" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    assert!(!layout.svg.contains("Hidden"), "{}", layout.svg);
    Ok(())
}

#[test]
fn grid_flow_positions_items_in_declared_columns() -> TestResult {
    let mut session = start(
        r#"<style>
main { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; width: 280px; }
.card { height: 40px; }
#first { background: #ef4444; }
#second { background: #3b82f6; }
</style>
<main><section id=first class=card>First</section><section id=second class=card>Second</section></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="8" y="8" width="134" height="40" fill="#ef4444"/>"##),
        "{}",
        layout.svg
    );
    assert!(
        layout
            .svg
            .contains(r##"<rect x="154" y="8" width="134" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn grid_default_alignment_stretches_each_item_to_its_own_row_height() -> TestResult {
    let mut session = start(
        r#"<main style="display:grid;grid-template-columns:100px 100px;gap:10px;width:210px">
<section style="padding:4px;background:#ef4444">A</section>
<section style="padding:24px 4px;background:#35a853">B</section>
<section style="padding:4px;background:#2457d6">C</section>
<section style="padding:12px 4px;background:#b99aff">D</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;
    let first = rect_height_for_fill(&layout.svg, "#ef4444")?;
    let second = rect_height_for_fill(&layout.svg, "#35a853")?;
    let third = rect_height_for_fill(&layout.svg, "#2457d6")?;
    let fourth = rect_height_for_fill(&layout.svg, "#b99aff")?;

    assert!((first - second).abs() < 0.1, "{}", layout.svg);
    assert!((third - fourth).abs() < 0.1, "{}", layout.svg);
    assert!(first > third, "{}", layout.svg);
    Ok(())
}

#[test]
fn grid_flow_preserves_fixed_and_fractional_track_sizes() -> TestResult {
    let mut session = start(
        r#"<style>
main { display: grid; grid-template-columns: 100px 1fr; gap: 12px; width: 280px; }
.card { height: 40px; }
#first { background: #ef4444; }
#second { background: #3b82f6; }
</style>
<main><section id=first class=card>First</section><section id=second class=card>Second</section></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="8" y="8" width="100" height="40" fill="#ef4444"/>"##),
        "{}",
        layout.svg
    );
    assert!(
        layout
            .svg
            .contains(r##"<rect x="120" y="8" width="168" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn grid_flow_positions_minmax_fraction_tracks_in_columns() -> TestResult {
    let mut session = start(
        r#"<style>
main { display: grid; grid-template-columns: minmax(80px, 1fr) minmax(120px, 1fr); gap: 10px; width: 310px; }
.card { height: 40px; }
#first { background: #ef4444; }
#second { background: #3b82f6; }
</style>
<main><section id=first class=card>First</section><section id=second class=card>Second</section></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="8" y="8" width="147" height="40" fill="#ef4444"/>"##),
        "{}",
        layout.svg
    );
    assert!(
        layout
            .svg
            .contains(r##"<rect x="165" y="8" width="147" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn grid_flow_resolves_percentage_item_width_once() -> TestResult {
    let mut session = start(
        r#"<main style="display:grid; grid-template-columns:1fr; width:280px">
<section style="width:50%; height:40px; background:#3b82f6">Half width</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="8" y="8" width="140" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn consecutive_inline_block_links_share_a_row_and_keep_exact_hit_boxes() -> TestResult {
    let mut session = start(
        r##"<style>
main { width: 280px; }
a { display: inline-block; margin-right: 8px; padding: 6px; background: #b99aff; }
</style>
<main><a href="#one">First link</a> <a href="#two">Second link</a></main>"##,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert_eq!(layout.hit_targets.len(), 2);
    let first = &layout.hit_targets[0];
    let second = &layout.hit_targets[1];
    assert!(second.x >= first.x + first.width + 8.0);
    assert_eq!(second.y, first.y);
    assert!(first.width < 120.0);
    assert!(second.width < 120.0);
    Ok(())
}

#[test]
fn default_link_keeps_a_click_target_after_inline_flow_layout() -> TestResult {
    let mut session = start("<a href=guide/next.html>Next</a>")?;
    let layout = session.layout().map_err(to_string)?;

    assert_eq!(layout.hit_targets.len(), 1, "{}", layout.svg);
    assert_eq!(
        layout.hit_targets[0].node_id,
        session.hit_targets[0].node_id
    );
    Ok(())
}

#[test]
fn nested_phrasing_content_shares_one_inline_line_inside_a_flex_item() -> TestResult {
    let mut session = start(
        r#"<main style="display:flex;align-items:center;width:280px">
<span><b>2</b> / <span>14</span></span>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    let first_y = text_baseline_for(&layout.svg, "2")?;
    let separator_y = text_baseline_for(&layout.svg, "/")?;
    let total_y = text_baseline_for(&layout.svg, "14")?;

    assert_eq!(separator_y, first_y, "{}", layout.svg);
    assert_eq!(total_y, first_y, "{}", layout.svg);
    Ok(())
}

#[test]
fn anonymous_text_is_centered_inside_a_fixed_size_flex_badge() -> TestResult {
    let mut session = start(
        r#"<style>
html, body { margin: 0; }
.badge {
  width: 46px; height: 46px; border-radius: 50%;
  background: #1e3a8a; color: #fff;
  display: flex; align-items: center; justify-content: center;
  font-family: Arial, sans-serif; font-weight: 700; font-size: 20px;
}
</style><div class="badge">1</div>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;
    let text_x = text_x_for(&layout.svg, "1")?;
    let text_y = text_baseline_for(&layout.svg, "1")?;

    assert!((text_x - 17.4375).abs() <= 1.5, "{}", layout.svg);
    assert!((text_y - 28.0).abs() <= 2.0, "{}", layout.svg);
    Ok(())
}

#[test]
fn anonymous_text_is_centered_inside_a_border_box_flex_pager_button() -> TestResult {
    let mut session = start(
        r#"<style>
* { box-sizing: border-box; }
html, body { margin: 0; }
button {
  width: 40px; height: 40px; border-radius: 50%;
  border: 1px solid #bfbfbf; background: #fff; color: #1a1a1a;
  font-family: Arial, sans-serif; font-size: 18px;
  display: flex; align-items: center; justify-content: center;
}
</style><button>‹</button>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;
    let text_x = text_x_for(&layout.svg, "‹")?;
    let text_y = text_baseline_for(&layout.svg, "‹")?;
    assert_eq!(layout.hit_targets.len(), 1, "{}", layout.svg);
    let target = &layout.hit_targets[0];

    assert_eq!(
        [target.width, target.height],
        [40.0, 40.0],
        "{}",
        layout.svg
    );
    assert!((text_x - 17.0).abs() <= 1.0, "{}", layout.svg);
    assert!((text_y - 25.0).abs() <= 2.0, "{}", layout.svg);
    Ok(())
}

#[test]
fn styled_inline_text_can_fragment_across_browser_line_boundaries() -> TestResult {
    let mut session = start_with_viewport(
        r#"<main style='width:220px;font-family:"Noto Sans JP","Hiragino Kaku Gothic ProN","Hiragino Sans","Yu Gothic",Meiryo,system-ui,sans-serif;font-size:19px;line-height:1.65;font-feature-settings:"palt" 1'>選択 ON で検索できることを確認したのち、選択 OFF で同じツールを呼び出すと <em style="color:#2c4ac6;font-style:normal;font-weight:600">MCP Hub がツール呼び出しを拒否</em> する様子を見せる（動的ツール制御の実演）</main>"#,
        1230,
        867,
    )?;
    let layout = session.layout().map_err(to_string)?;
    let emphasized = text_fragments_for_fill(&layout.svg, "#2c4ac6")?;

    assert!(emphasized.len() >= 2, "{}", layout.svg);
    assert_eq!(
        emphasized.concat(),
        "MCP Hub がツール呼び出しを拒否",
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn long_text_after_inline_code_uses_the_remaining_line_before_wrapping() -> TestResult {
    let mut session = start(
        r#"<main style="width:180px"><code style="font-family:monospace">/d sdd</code> next remaining words</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;
    let code_y = text_baseline_for(&layout.svg, "/d sdd")?;
    let first_text_y = text_baseline_for(&layout.svg, "next remaining")?;
    let wrapped_text_y = text_baseline_for(&layout.svg, "words")?;

    assert_eq!(first_text_y, code_y, "{}", layout.svg);
    assert!(wrapped_text_y > first_text_y, "{}", layout.svg);
    Ok(())
}

#[test]
fn nested_flex_intrinsic_width_keeps_page_indicator_inline_with_appendix_tag() -> TestResult {
    let mut session = start(
        r#"<main style="display:flex;justify-content:space-between;width:300px">
<div style="display:flex;align-items:center;gap:4px">
<button style="box-sizing:border-box;width:40px">‹</button>
<button style="box-sizing:border-box;width:40px">›</button>
<span><b>12</b> / <span>14</span></span>
<span style="display:inline-flex;padding:3px 6px">APPENDIX</span>
</div>
<div>Hint</div>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    let current_y = text_baseline_for(&layout.svg, "12")?;
    assert_eq!(
        text_baseline_for(&layout.svg, "/")?,
        current_y,
        "{}",
        layout.svg
    );
    assert_eq!(
        text_baseline_for(&layout.svg, "14")?,
        current_y,
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn fixed_slideshow_navigation_keeps_page_indicator_on_one_line() -> TestResult {
    let mut session = start_with_viewport(FIXED_SLIDESHOW_NAVIGATION_DOCUMENT, 1_382, 744)?;
    let layout = session.layout().map_err(to_string)?;

    let current_y = text_baseline_for(&layout.svg, "4")?;
    assert_eq!(
        text_baseline_for(&layout.svg, "/")?,
        current_y,
        "{}",
        layout.svg
    );
    assert_eq!(
        text_baseline_for(&layout.svg, "14")?,
        current_y,
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn markerless_flex_list_applies_its_declared_column_gap() -> TestResult {
    let mut session = start(
        r#"<ul style="list-style:none;display:flex;flex-direction:column;gap:16px;margin:0;padding:0;font-size:20px;line-height:30px">
<li>First row</li><li>Second row</li>
</ul>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    let first_y = text_baseline_for(&layout.svg, "First row")?;
    let second_y = text_baseline_for(&layout.svg, "Second row")?;
    assert!((second_y - first_y - 46.0).abs() < 0.01, "{}", layout.svg);
    Ok(())
}

#[test]
fn anonymous_flex_text_uses_its_intrinsic_width_when_space_is_available() -> TestResult {
    let mut session = start(
        r#"<div style="display:flex;align-items:baseline;gap:10px;width:300px;font-size:16px">
<span style="font-size:20px">①</span>OSS fork の upstream 追従運用
</div>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout.svg.contains(">OSS fork の upstream 追従運用</text>"),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn per_edge_borders_override_uniform_border_and_contribute_to_box_geometry() -> TestResult {
    let mut session = start(
        r#"<main style="width:100px;height:40px;border:1px solid #999999;border-left:6px solid #111111;border-left-color:#2457d6;border-top:3px solid #e11d48;background:#ffffff">Edge content</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"stroke="#2457d6" stroke-width="6""##),
        "{}",
        layout.svg
    );
    assert!(
        layout
            .svg
            .contains(r##"stroke="#e11d48" stroke-width="3""##),
        "{}",
        layout.svg
    );
    assert!(
        layout.svg.contains(r#"<text x="14""#),
        "left border must offset content: {}",
        layout.svg
    );
    Ok(())
}

#[test]
fn percentage_border_radius_resolves_against_both_box_axes() -> TestResult {
    let mut session = start(
        r#"<div style="width:40px;height:40px;border-radius:50%;background:#2457d6"></div>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout.svg.contains(
            r##"<rect x="8" y="8" width="40" height="40" rx="20" ry="20" fill="#2457d6"/>"##
        ),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn positive_z_index_paints_positioned_content_above_later_normal_flow() -> TestResult {
    let mut session = start(
        r#"<div style="position:fixed;inset:0;height:3px;background:#2457d6;z-index:900"></div>
<main style="height:120px;background:#f4f5f7"></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    let normal = layout
        .svg
        .find(r##"fill="#f4f5f7""##)
        .ok_or_else(|| format!("missing normal layer: {}", layout.svg))?;
    let overlay = layout
        .svg
        .find(r##"fill="#2457d6""##)
        .ok_or_else(|| format!("missing positioned layer: {}", layout.svg))?;
    assert!(overlay > normal, "{}", layout.svg);
    Ok(())
}

#[test]
fn flex_wrap_honors_item_min_width() -> TestResult {
    let mut session = start(
        r#"<main style="display:flex;flex-wrap:wrap;gap:10px;width:280px">
<section style="flex:1;min-width:220px;height:30px;background:#ef4444"></section>
<section style="flex:1;min-width:220px;height:30px;background:#3b82f6"></section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    let first_y = rect_y_for_fill(&layout.svg, "#ef4444")?;
    let second_y = rect_y_for_fill(&layout.svg, "#3b82f6")?;
    assert!(second_y >= first_y + 40.0, "{}", layout.svg);
    Ok(())
}

#[test]
fn block_children_inside_flex_column_keep_the_assigned_content_width() -> TestResult {
    let mut session = start(
        r#"<main style="display:flex;flex-direction:column;width:280px">
<section style="padding:20px 24px;border:1px solid #dddddd;border-left:4px solid #2457d6">
<div style="font-size:22px;font-weight:700;margin-bottom:10px">ゴール</div>
<div style="font-size:20px">自然言語で横断分析する</div>
</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(layout.svg.contains(">ゴール</text>"), "{}", layout.svg);
    assert!(!layout.svg.contains(">ゴ</text>"), "{}", layout.svg);
    Ok(())
}

#[test]
fn flex_figure_constrains_an_image_to_its_assigned_content_height() -> TestResult {
    let mut session = start(
        r#"<style>
body { margin: 0; }
.slide-body { display: flex; flex-direction: column; width: 200px; height: 100px; }
.fig { flex: 1; min-height: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 10px; }
.fig-img { max-width: 100%; max-height: 100%; }
</style>
<div class="slide-body"><div class="fig"><img class="fig-img" src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAGQAAABiAQAAAACOwEvkAAAAGUlEQVQ4y2P4jwQ+MIzyRnmjvFHeKI8KPAAhJu+FXHMNjwAAAABJRU5ErkJggg=="></div></div>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout.svg.contains(r#"width="82" height="80""#),
        "{}",
        layout.svg
    );
    assert!((82.0_f32 / 80.0 - 100.0 / 98.0).abs() < 0.005);
    Ok(())
}

#[test]
fn flex_auto_min_height_keeps_heading_and_margin_outside_flexible_content() -> TestResult {
    let mut session = start(FLEX_AUTO_MIN_HEIGHT_DOCUMENT)?;
    let nodes = session
        .runtime
        .interactive_nodes_at_width(320.0)
        .map_err(to_string)?;
    let heading_style = heading_style(&nodes).ok_or_else(|| "missing heading style".to_string())?;
    assert!(heading_style.contains("margin-top: 0"), "{heading_style}");
    assert!(
        heading_style.contains("margin-bottom: 20px"),
        "{heading_style}"
    );
    let layout = session.layout().map_err(to_string)?;

    assert_eq!(
        rect_y_for_fill(&layout.svg, "#2457d6")?,
        0.0,
        "{}",
        layout.svg
    );
    assert!(
        rect_y_for_fill(&layout.svg, "#ef4444")? >= 60.0,
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn slide_heading_wraps_with_browser_font_shaping_at_the_declared_content_width() -> TestResult {
    let mut session = start_with_viewport(SLIDE_HEADING_DOCUMENT, 1_382, 744)?;
    let layout = session.layout().map_err(to_string)?;
    let lines = svg_text_contents(&layout.svg);

    assert_eq!(
        lines,
        [
            "LibreChat fork to MCP Hub to Code Sandbox in three layers",
            "architecture"
        ],
        "{}",
        layout.svg,
    );
    assert!(
        !layout.svg.contains(
            ">LibreChat fork to MCP Hub to Code Sandbox in three layers architecture</text>"
        ),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn list_style_none_suppresses_markers_and_keeps_flex_item_children_aligned() -> TestResult {
    let mut session = start(
        r#"<ul style="list-style:none;width:280px">
<li style="display:flex;gap:14px"><span>①</span><span>Summary text</span></li>
</ul>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(!layout.svg.contains(">•</text>"), "{}", layout.svg);
    assert_eq!(
        text_baseline_for(&layout.svg, "①")?,
        text_baseline_for(&layout.svg, "Summary text")?,
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn nested_bold_inline_content_uses_its_own_intrinsic_flex_width() -> TestResult {
    let mut session = start(
        r#"<ul style="display:flex;flex-direction:column;gap:8px;list-style:none;width:280px">
<li style="display:flex;gap:14px"><span style="flex-shrink:0">③</span><span><b>Excel 帳票出力</b></span></li>
<li style="display:flex;gap:14px"><span style="flex-shrink:0">④</span><span><b>市場レポート・キャンペーン情報等の横断参照</b></span></li>
</ul>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;
    let lines = svg_text_contents(&layout.svg);
    assert!(
        lines.contains(&"Excel 帳票出力".to_string()),
        "{lines:?}\n{}",
        layout.svg
    );
    assert!(lines.len() >= 5, "{lines:?}\n{}", layout.svg);
    let wrapped = &lines[3..];
    assert_eq!(wrapped.len(), 2, "{lines:?}\n{}", layout.svg);
    assert_eq!(
        wrapped.concat(),
        "市場レポート・キャンペーン情報等の横断参照",
        "{lines:?}\n{}",
        layout.svg
    );
    assert!(
        wrapped.iter().all(|line| line.chars().count() > 1),
        "{lines:?}\n{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn opacity_wraps_the_complete_element_paint() -> TestResult {
    let mut session =
        start(r#"<div style="width:40px;height:40px;background:#2457d6;opacity:0.3">Dim</div>"#)?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout.svg.contains(r#"<g opacity="0.3"><rect"#),
        "{}",
        layout.svg
    );
    assert!(layout.svg.contains("</text></g>"), "{}", layout.svg);
    Ok(())
}

#[test]
fn disabled_pseudo_class_applies_stateful_control_style() -> TestResult {
    let mut session = start(
        r#"<style>button:disabled { opacity: 0.3; }</style><button disabled>Previous</button>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout.svg.contains(r#"<g opacity="0.3">"#),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn text_transform_and_font_features_reach_the_painted_svg() -> TestResult {
    let mut session = start(
        r#"<p style='font-family:"Hiragino Sans";text-transform:uppercase;font-feature-settings:"palt" 1'>（LibreChat）</p>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout.svg.contains(">（LIBRECHAT）</text>"),
        "{}",
        layout.svg
    );
    assert!(
        layout.svg.contains(" dx=\"") || layout.svg.contains(" textLength=\""),
        "{}",
        layout.svg
    );
    assert!(!layout.svg.contains(">ibreChat</"), "{}", layout.svg);
    assert!(
        layout
            .svg
            .contains("font-feature-settings=\"&quot;palt&quot; 1\""),
        "{}",
        layout.svg
    );
    Ok(())
}

fn rect_y_for_fill(svg: &str, fill: &str) -> TestResult<f32> {
    let marker = format!(r#" fill="{fill}""#);
    let marker_start = svg
        .find(&marker)
        .ok_or_else(|| format!("missing fill {fill}: {svg}"))?;
    let rect_start = svg[..marker_start]
        .rfind("<rect ")
        .ok_or_else(|| format!("missing rect for {fill}: {svg}"))?;
    let element = &svg[rect_start..marker_start];
    let y_start = element
        .find(" y=\"")
        .ok_or_else(|| format!("missing y for {fill}: {element}"))?
        + 4;
    let y_end = element[y_start..]
        .find('"')
        .ok_or_else(|| format!("unterminated y for {fill}: {element}"))?
        + y_start;
    element[y_start..y_end].parse::<f32>().map_err(to_string)
}

fn text_fragments_for_fill(svg: &str, fill: &str) -> TestResult<Vec<String>> {
    let root = xmltree::Element::parse(svg.as_bytes()).map_err(|error| error.to_string())?;
    let mut fragments = Vec::new();
    collect_text_fragments_for_fill(&root, fill, &mut fragments)?;
    Ok(fragments)
}

fn collect_text_fragments_for_fill(
    element: &xmltree::Element,
    fill: &str,
    fragments: &mut Vec<String>,
) -> TestResult {
    if element.name == "text"
        && element
            .attributes
            .get("fill")
            .is_some_and(|value| value == fill)
    {
        let text = element
            .get_text()
            .ok_or_else(|| format!("text element with fill {fill} has no content"))?;
        fragments.push(text.into_owned());
    }
    for child in &element.children {
        if let xmltree::XMLNode::Element(child) = child {
            collect_text_fragments_for_fill(child, fill, fragments)?;
        }
    }
    Ok(())
}

fn rect_width_for_fill(svg: &str, fill: &str) -> TestResult<f32> {
    rect_attribute_for_fill(svg, fill, "width")
}

fn rect_height_for_fill(svg: &str, fill: &str) -> TestResult<f32> {
    rect_attribute_for_fill(svg, fill, "height")
}

fn colored_card_dimensions(svg: &str) -> TestResult<([f32; 3], [f32; 3])> {
    let fills = ["#ef4444", "#35a853", "#2457d6"];
    let widths = fills
        .map(|fill| rect_width_for_fill(svg, fill))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let heights = fills
        .map(|fill| rect_height_for_fill(svg, fill))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        widths
            .try_into()
            .map_err(|_| "expected three card widths".to_string())?,
        heights
            .try_into()
            .map_err(|_| "expected three card heights".to_string())?,
    ))
}

fn rect_attribute_for_fill(svg: &str, fill: &str, attribute: &str) -> TestResult<f32> {
    let marker = format!(r#" fill="{fill}""#);
    let marker_start = svg
        .find(&marker)
        .ok_or_else(|| format!("missing fill {fill}: {svg}"))?;
    let rect_start = svg[..marker_start]
        .rfind("<rect ")
        .ok_or_else(|| format!("missing rect for {fill}: {svg}"))?;
    let element = &svg[rect_start..marker_start];
    let attribute_marker = format!(r#" {attribute}=""#);
    let value_start = element
        .find(&attribute_marker)
        .ok_or_else(|| format!("missing {attribute} for {fill}: {element}"))?
        + attribute_marker.len();
    let value_end = element[value_start..]
        .find('"')
        .ok_or_else(|| format!("unterminated {attribute} for {fill}: {element}"))?
        + value_start;
    element[value_start..value_end]
        .parse::<f32>()
        .map_err(to_string)
}

fn text_baseline_for(svg: &str, text: &str) -> TestResult<f32> {
    let marker = format!(">{text}</text>");
    let marker_start = svg
        .find(&marker)
        .ok_or_else(|| format!("missing text marker {marker}: {svg}"))?;
    let text_start = svg[..marker_start]
        .rfind("<text ")
        .ok_or_else(|| format!("missing text element for {text}: {svg}"))?;
    let element = &svg[text_start..marker_start];
    let y_start = element
        .find(" y=\"")
        .ok_or_else(|| format!("missing y attribute for {text}: {element}"))?
        + 4;
    let y_end = element[y_start..]
        .find('"')
        .ok_or_else(|| format!("unterminated y attribute for {text}: {element}"))?
        + y_start;
    element[y_start..y_end].parse::<f32>().map_err(to_string)
}

fn text_x_for(svg: &str, text: &str) -> TestResult<f32> {
    let marker = format!(">{text}</text>");
    let marker_start = svg
        .find(&marker)
        .ok_or_else(|| format!("missing text marker {marker}: {svg}"))?;
    let text_start = svg[..marker_start]
        .rfind("<text ")
        .ok_or_else(|| format!("missing text element for {text}: {svg}"))?;
    let element = &svg[text_start..marker_start];
    let x_start = element
        .find(" x=\"")
        .ok_or_else(|| format!("missing x attribute for {text}: {element}"))?
        + 4;
    let x_end = element[x_start..]
        .find('"')
        .ok_or_else(|| format!("unterminated x attribute for {text}: {element}"))?
        + x_start;
    element[x_start..x_end].parse::<f32>().map_err(to_string)
}

fn svg_text_contents(svg: &str) -> Vec<String> {
    let mut remaining = svg;
    let mut contents = Vec::new();
    while let Some(text_start) = remaining.find("<text ") {
        remaining = &remaining[text_start..];
        let Some(content_start) = remaining.find('>') else {
            break;
        };
        let Some(content_end) = remaining.find("</text>") else {
            break;
        };
        let mut content = String::new();
        let mut inside_tag = false;
        for character in remaining[content_start + 1..content_end].chars() {
            match character {
                '<' => inside_tag = true,
                '>' => inside_tag = false,
                value if !inside_tag => content.push(value),
                _ => {}
            }
        }
        contents.push(content);
        remaining = &remaining[content_end + "</text>".len()..];
    }
    contents
}

#[test]
fn percentage_width_reflows_against_resized_viewport() -> TestResult {
    let mut session = start(r#"<main style="width:50%; height:40px; background:#123456"></main>"#)?;
    let initial = session.layout().map_err(to_string)?;
    assert!(
        initial
            .svg
            .contains(r##"<rect x="8" y="8" width="152" height="40" fill="#123456"/>"##),
        "{}",
        initial.svg
    );

    session
        .resize(HtmlBrowserViewport::new(480, 240, 1.0).map_err(to_string)?)
        .map_err(to_string)?;
    let resized = session.layout().map_err(to_string)?;
    assert!(
        resized
            .svg
            .contains(r##"<rect x="8" y="8" width="232" height="40" fill="#123456"/>"##),
        "{}",
        resized.svg
    );
    Ok(())
}

#[test]
fn raster_dimension_mismatch_is_a_typed_runtime_failure() -> TestResult {
    let session = start("<p>frame</p>")?;

    assert!(matches!(
        session.validate_raster_dimensions(319, 240),
        Err(crate::renderer::backends::html_browser::HtmlBrowserError::RuntimeFailure { error })
            if error.contains("319x240") && error.contains("320x240")
    ));
    Ok(())
}

#[test]
fn session_start_validates_mutated_sources_and_viewports() -> TestResult {
    assert!(matches!(
        super::super::HtmlInteractiveSession::start(oversized_source()?, test_viewport()?),
        Err(HtmlBrowserError::SourceTooLarge { .. })
    ));
    assert!(matches!(
        super::super::HtmlInteractiveSession::start(
            HtmlBrowserSource::new("<p>frame</p>", "https://example.test/docs/index.html")
                .map_err(to_string)?,
            HtmlBrowserViewport {
                width: 0,
                height: TEST_VIEWPORT_HEIGHT,
                device_scale_factor: 1.0,
            },
        ),
        Err(HtmlBrowserError::InvalidViewport)
    ));
    Ok(())
}

#[test]
fn high_density_viewport_uses_logical_css_layout_and_physical_frame() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<main style='background:#123456'><p>Retina frame</p></main>",
        "https://example.test/docs/index.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(640, 480, 2.0).map_err(to_string)?;
    let mut session =
        super::super::HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "retina frame must exist".to_string())?;

    assert_eq!(frame.pixels.len(), 640 * 480 * 4);
    let layout = session.layout().map_err(to_string)?;
    assert!(
        layout
            .svg
            .starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="480" viewBox="0 0 320 240">"#),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn border_color_uses_the_color_component_not_the_border_style() {
    assert_eq!(
        super::super::document::border_color("1px solid #93b4cf"),
        Some("#93b4cf".to_string())
    );
    assert_eq!(
        super::super::document::border_color("2px dashed red"),
        Some("red".to_string())
    );
    assert_eq!(
        super::super::document::border_color("1px solid rgba(175, 47, 47, 0.2)"),
        Some("rgba(175, 47, 47, 0.2)".to_string())
    );
    assert_eq!(super::super::document::border_color("solid"), None);
}

fn metadata_document() -> &'static str {
    r#"<!doctype html><html><head><title>Hidden</title><style>h1 { color: #0b74c7; }</style><script>window.hidden = true;</script></head><body><h1>Visible title</h1><p>Visible body</p></body></html>"#
}

fn styled_link_document() -> &'static str {
    r#"<style>
summary { background: #dbeafe; border: 1px solid #2563eb; padding: 8px; }
a { background: #ede9fe; border: 1px solid #7c3aed; padding: 8px; }
</style><details><summary id=more>More</summary><p>Hidden panel</p></details><a id=next href=next.html style="color:#4c1d95">Next</a>"#
}

fn oversized_source() -> TestResult<HtmlBrowserSource> {
    let mut source = HtmlBrowserSource::new("<p>frame</p>", "https://example.test/docs/index.html")
        .map_err(to_string)?;
    source.raw_html = "x".repeat(HTML_BROWSER_MAX_SOURCE_BYTES + 1);
    Ok(source)
}

fn test_viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0).map_err(to_string)
}

fn heading_style(nodes: &[HtmlDocumentNode]) -> Option<&str> {
    nodes.iter().find_map(|node| match node {
        HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } => {
            if tag == "h1" {
                attributes
                    .iter()
                    .find(|(name, _)| name == "style")
                    .map(|(_, value)| value.as_str())
            } else {
                heading_style(children)
            }
        }
        HtmlDocumentNode::Text(_) => None,
    })
}

fn assert_target_exists(
    session: &mut super::super::HtmlInteractiveSession,
    id: &str,
) -> TestResult {
    let node_id = session
        .runtime
        .node_for_element_id(id)
        .ok_or_else(|| format!("{id} node must exist"))?
        .0;
    assert!(
        session
            .hit_targets
            .iter()
            .any(|target| target.node_id == node_id)
    );
    Ok(())
}

fn element_box_for_id(
    session: &mut super::super::HtmlInteractiveSession,
    id: &str,
) -> TestResult<super::super::types::ElementBox> {
    let node_id = session
        .runtime
        .node_for_element_id(id)
        .ok_or_else(|| format!("{id} node is missing"))?
        .0;
    session
        .element_boxes
        .iter()
        .find(|element| element.node_id == node_id)
        .cloned()
        .ok_or_else(|| format!("{id} layout box is missing"))
}
