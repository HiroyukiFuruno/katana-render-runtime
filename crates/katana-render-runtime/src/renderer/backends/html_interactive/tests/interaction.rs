use super::super::types::HitTargetKind;
use super::support::{
    TestResult, click_element, click_first_target, frame_contains_rgb, frame_matching_rgb_pixels,
    has_open_details, input_value, start, to_string,
};
use crate::renderer::backends::html_browser::{
    HtmlBrowserError, HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport,
};

const NO_HORIZONTAL_SCROLL: f32 = 0.0;
const UNIT_SCROLL_DELTA: f32 = 1.0;
const STICKY_FRAGMENT_DOCUMENT: &str = r#"<style>
html, body { margin: 0; }
.app { display: flex; align-items: flex-start; }
.sidebar { width: 80px; flex-shrink: 0; height: 100vh; background: #0f172a; position: sticky; top: 0; }
.main { flex: 1; }
.spacer { height: 640px; }
.target { height: 100px; background: #e8c7ff; }
.tail { height: 400px; }
</style>
<div class=app>
  <aside class=sidebar>TOC</aside>
  <main class=main><div class=spacer></div><section id=s15 class=target>Target</section><div class=tail></div></main>
</div>"#;
const BLOCK_STICKY_FRAGMENT_DOCUMENT: &str = r#"<style>
html, body { margin: 0; }
.sidebar { width: 80px; height: 100vh; background: #0f172a; position: sticky; top: 0; }
main { margin-left: 80px; }
.spacer { height: 640px; }
.target { height: 100px; background: #e8c7ff; }
.tail { height: 400px; }
</style>
<aside class=sidebar>TOC</aside>
<main><div class=spacer></div><section id=s15 class=target>Target</section><div class=tail></div></main>"#;

#[test]
fn button_click_runs_v8_handler_and_repaints() -> TestResult {
    let mut session = start(button_document())?;
    let before = frame_generation(&session, "initial frame")?;
    click_first_target(&mut session)?;
    let after = frame_generation(&session, "updated frame")?;
    assert!(after > before);
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains("Done")
    );
    Ok(())
}

#[test]
fn host_click_uses_dom_event_capture_target_and_stopped_bubbling() -> TestResult {
    let mut session = start(
        r#"<div id=parent><button id=child>Run</button></div><p id=order></p><script>
const parent = document.getElementById('parent');
const child = document.getElementById('child');
const order = document.getElementById('order');
parent.addEventListener('click', () => { order.textContent += 'capture|'; }, true);
child.addEventListener('click', (event) => {
  order.textContent += 'target|';
  event.stopPropagation();
});
parent.addEventListener('click', () => { order.textContent += 'bubble|'; });
</script>"#,
    )?;

    click_element(&mut session, "child")?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains("capture|target|"), "{snapshot}");
    assert!(!snapshot.contains("bubble|"), "{snapshot}");
    Ok(())
}

#[test]
fn class_list_mutation_recascades_css_after_click() -> TestResult {
    let mut session = start(
        r#"<style>button.active { background: #35a853; color: #ffffff; }</style><button id=run>Run</button><script>document.getElementById('run').addEventListener('click', function () { this.classList.toggle('active'); });</script>"#,
    )?;
    let before = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert!(!frame_contains_rgb(before, [53, 168, 83]));

    click_element(&mut session, "run")?;

    let after = session
        .latest_frame()
        .ok_or_else(|| "updated frame must exist".to_string())?;
    assert!(frame_contains_rgb(after, [53, 168, 83]));
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains(r#"class="active""#), "{snapshot}");
    Ok(())
}

#[test]
fn pointer_move_recascades_hover_for_the_element_and_its_ancestors() -> TestResult {
    let mut session = start(
        r#"<style>
.card:hover button:not(:disabled) { background: #35a853; color: #ffffff; }
</style>
<div id=card class=card><button id=active>Next</button></div>
<button id=disabled disabled>Disabled</button>"#,
    )?;
    let before = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert!(!frame_contains_rgb(before, [53, 168, 83]));

    move_pointer_outside_viewport(&mut session)?;
    move_pointer_to_element(&mut session, "active")?;

    let hovered = session
        .latest_frame()
        .ok_or_else(|| "hovered frame must exist".to_string())?;
    assert!(frame_contains_rgb(hovered, [53, 168, 83]));

    move_pointer_to_element(&mut session, "disabled")?;

    let disabled = session
        .latest_frame()
        .ok_or_else(|| "disabled hover frame must exist".to_string())?;
    assert!(!frame_contains_rgb(disabled, [53, 168, 83]));
    Ok(())
}

#[test]
fn pointer_move_surfaces_a_stale_layout_node_path() -> TestResult {
    let mut session = start("<div>target</div>")?;
    let element = session
        .element_boxes
        .last_mut()
        .ok_or_else(|| "element box must exist".to_string())?;
    element.node_id = u64::MAX;
    let pointer = (
        element.x + element.width / 2.0,
        element.y + element.height / 2.0,
    );

    let result = session.dispatch_input(HtmlBrowserInput::PointerMove {
        x: pointer.0,
        y: pointer.1,
    });

    assert!(matches!(
        result,
        Err(HtmlBrowserError::RuntimeFailure { .. })
    ));
    Ok(())
}

#[test]
fn slide_deck_repaints_for_document_keys_fixed_controls_and_surface_clicks() -> TestResult {
    let mut session = start(SLIDE_DECK_DOCUMENT)?;
    let initial = session
        .latest_frame()
        .ok_or_else(|| "initial slide frame must exist".to_string())?;
    assert!(frame_contains_rgb(initial, [23, 51, 130]));
    assert!(!frame_contains_rgb(initial, [53, 168, 83]));
    assert_slide_state(&session, "1", "slide cover active", "slide")?;

    session
        .dispatch_input(HtmlBrowserInput::KeyDown {
            key: "ArrowRight".to_string(),
        })
        .map_err(to_string)?;
    let keyed = session
        .latest_frame()
        .ok_or_else(|| "keyboard slide frame must exist".to_string())?;
    assert!(frame_contains_rgb(keyed, [53, 168, 83]));
    assert_slide_state(&session, "2", "slide cover", "slide active")?;

    click_element(&mut session, "prev")?;
    assert_slide_state(&session, "1", "slide cover active", "slide")?;
    click_element(&mut session, "first")?;
    assert_slide_state(&session, "2", "slide cover", "slide active")?;
    Ok(())
}

const REPEATED_SLIDE_DOCUMENT_TEMPLATE: &str = r#"<style>
.slide { display:none; position:absolute; inset:0; width:100vw; height:100vh; }
.slide.active { display:block; }
</style>
<section id=one class="slide active">One<!-- filler --></section>
<section id=two class="slide">Two<!-- filler --></section>
<section id=three class="slide">Three</section>
<section id=four class="slide">Four</section>
<span id=page>1</span>
<script>
var slides = Array.prototype.slice.call(document.querySelectorAll('.slide'));
var index = 0;
function render() {
  slides.forEach(function (slide, position) { slide.classList.toggle('active', position === index); });
  document.getElementById('page').textContent = String(index + 1);
}
slides.forEach(function (slide) {
  slide.addEventListener('click', function (event) {
    if (event.target.closest('.blocked')) return;
    index = Math.min(slides.length - 1, index + 1);
    render();
  });
});
</script>"#;

#[test]
fn repeated_slide_surface_clicks_keep_every_listener_and_advance_all_slides() -> TestResult {
    let filler = (0..80).map(|_| "<i></i>").collect::<String>();
    let document = REPEATED_SLIDE_DOCUMENT_TEMPLATE.replace("<!-- filler -->", &filler);
    let mut session = start(&document)?;

    for (active, page) in [("one", "2"), ("two", "3"), ("three", "4")] {
        click_element(&mut session, active)?;
        let snapshot = session.runtime.snapshot().map_err(to_string)?;
        assert!(
            snapshot.contains(&format!(r#"id="page">{page}</span>"#)),
            "{snapshot}"
        );
    }
    Ok(())
}

fn assert_slide_state(
    session: &super::super::HtmlInteractiveSession,
    page: &str,
    first_class: &str,
    second_class: &str,
) -> TestResult {
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains(&format!(r#"id="page">{page}</span>"#)),
        "{snapshot}"
    );
    assert!(
        snapshot.contains(&format!(r#"id="first" class="{first_class}""#)),
        "{snapshot}"
    );
    assert!(
        snapshot.contains(&format!(r#"id="second" class="{second_class}""#)),
        "{snapshot}"
    );
    Ok(())
}

const SLIDE_DECK_DOCUMENT: &str = r##"<style>
* { box-sizing: border-box; }
html, body { margin: 0; width: 100%; height: 100%; overflow: hidden; }
.deck { position: relative; width: 100vw; height: 100vh; }
.slide { position: absolute; inset: 0; display: none; flex-direction: column; padding: 24px 28px 40px; }
.slide.active { display: flex; }
.cover { background: linear-gradient(155deg, #173382 0%, #2c4ac6 62%, #3952ff 100%); color: #fff; }
.inner { display: flex; flex-direction: column; flex: 1; min-height: 0; justify-content: center; }
h1 { font-size: clamp(20px, 8vw, 32px); line-height: 1.2; }
.success { background: #35a853; width: 120px; height: 60px; }
.nav { position: fixed; left: 0; right: 0; bottom: 0; height: 32px; background: #fff; display: flex; }
</style>
<div class="deck">
  <section id="first" class="slide cover active"><div class="inner"><h1>First<br>slide</h1></div></section>
  <section id="second" class="slide"><div class="success">Second slide</div></section>
</div>
<div class="nav"><button id="prev">Prev</button><span id="page">1</span></div>
<script>
var slides = Array.prototype.slice.call(document.querySelectorAll('.slide'));
var index = 0;
function renderSlide() {
  slides.forEach(function (slide, position) { slide.classList.toggle('active', position === index); });
  document.getElementById('page').textContent = String(index + 1);
}
function go(next) { index = Math.max(0, Math.min(slides.length - 1, next)); renderSlide(); }
document.getElementById('prev').addEventListener('click', function (event) { event.stopPropagation(); go(index - 1); });
document.addEventListener('keydown', function (event) { if (event.key === 'ArrowRight') go(index + 1); });
slides.forEach(function (slide) {
  slide.addEventListener('click', function (event) {
    if (event.target.closest('.no-advance')) return;
    go(index + 1);
  });
});
renderSlide();
</script>"##;

#[test]
fn custom_property_style_mutation_recascades_and_repaints_after_click() -> TestResult {
    let mut session = start(
        r#"<style>#card { --accent: #ef4444; background: var(--accent); width: 80px; height: 40px; }</style>
<div id=card>Card</div><button id=run>Update</button><script>
document.getElementById('run').addEventListener('click', () => {
  document.getElementById('card').style['--accent'] = '#35a853';
});
</script>"#,
    )?;
    let before = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert!(frame_contains_rgb(before, [239, 68, 68]));
    assert!(!frame_contains_rgb(before, [53, 168, 83]));

    click_element(&mut session, "run")?;

    let after = session
        .latest_frame()
        .ok_or_else(|| "updated frame must exist".to_string())?;
    assert!(frame_contains_rgb(after, [53, 168, 83]));
    assert!(!frame_contains_rgb(after, [239, 68, 68]));
    Ok(())
}

#[test]
fn summary_click_toggles_details_without_host_dom_logic() -> TestResult {
    let mut session = start("<details><summary>More</summary><p>Expanded content</p></details>")?;
    click_first_target(&mut session)?;
    let nodes = session.runtime.interactive_nodes().map_err(to_string)?;
    assert!(has_open_details(&nodes));
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains("Expanded content")
    );
    Ok(())
}

#[test]
fn text_input_updates_the_v8_backed_dom_and_frame() -> TestResult {
    let mut session = start("<label>Name<input value=initial></label>")?;
    click_first_target(&mut session)?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: " value".to_string(),
        })
        .map_err(to_string)?;
    let nodes = session.runtime.interactive_nodes().map_err(to_string)?;
    assert_eq!(input_value(&nodes), Some("initial value"));
    Ok(())
}

#[test]
fn checkbox_click_toggles_checked_before_the_javascript_click_listener() -> TestResult {
    let mut session = start(
        r#"<input id=choice type=checkbox style="width:32px;height:32px"><p id=status>unchecked</p><script>
const choice = document.getElementById('choice');
choice.addEventListener('click', () => {
  document.getElementById('status').textContent = choice.checked ? 'checked' : 'unchecked';
});
</script>"#,
    )?;

    click_element(&mut session, "choice")?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains("checked=\"\""), "{snapshot}");
    assert!(snapshot.contains(">checked</p>"), "{snapshot}");
    assert!(
        session
            .hit_targets
            .iter()
            .any(|target| matches!(target.kind, HitTargetKind::Checkbox))
    );
    Ok(())
}

#[test]
fn attribute_selectors_dataset_and_input_toggle_events_repaint_the_dom() -> TestResult {
    let mut session = start(event_document())?;
    assert_initial_status(&session)?;
    click_element(&mut session, "summary")?;
    assert_open_status(&session)?;
    update_event_input(&mut session)?;
    assert_event_input_snapshot(&session)
}

#[test]
fn focused_input_dispatches_keyboard_text_change_and_blur_in_order() -> TestResult {
    let mut session = start(
        r#"<input id=field><p id=status></p><script>
const field = document.getElementById('field');
const status = document.getElementById('status');
for (const type of ['focus', 'keydown', 'keyup', 'input', 'change', 'blur']) {
  field.addEventListener(type, (event) => {
    status.textContent += `${event.type}${event.key ? ':' + event.key : ''}|`;
  });
}
</script>"#,
    )?;

    click_element(&mut session, "field")?;
    click_element(&mut session, "field")?;
    dispatch_committed_input(&mut session)?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains("focus|keydown:A|input|keyup:A|change|blur|"),
        "{snapshot}"
    );
    Ok(())
}

#[test]
fn enter_commits_a_dirty_input_once_without_dropping_focus() -> TestResult {
    let mut session = start(
        r#"<input id=field><p id=status></p><script>
const field = document.getElementById('field');
const status = document.getElementById('status');
for (const type of ['input', 'keydown', 'keyup', 'change', 'blur']) {
  field.addEventListener(type, (event) => {
    status.textContent += `${event.type}${event.key ? ':' + event.key : ''}|`;
  });
}
</script>"#,
    )?;

    click_element(&mut session, "field")?;
    dispatch_enter_commit_sequence(&mut session)?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert_eq!(snapshot.matches("change|").count(), 1, "{snapshot}");
    assert!(
        snapshot.contains("input|keydown:Enter|change|keyup:Enter|blur|"),
        "{snapshot}"
    );
    Ok(())
}

fn dispatch_enter_commit_sequence(
    session: &mut super::super::HtmlInteractiveSession,
) -> TestResult {
    for input in [
        HtmlBrowserInput::Text {
            text: "committed".to_string(),
        },
        HtmlBrowserInput::KeyDown {
            key: "Enter".to_string(),
        },
        HtmlBrowserInput::KeyUp {
            key: "Enter".to_string(),
        },
        HtmlBrowserInput::Focus { focused: false },
    ] {
        session.dispatch_input(input).map_err(to_string)?;
    }
    Ok(())
}

#[test]
fn focused_input_is_blurred_on_blank_pointer_activation_and_text_stops_after_focus_loss()
-> TestResult {
    let mut session = blank_blur_session()?;
    click_element(&mut session, "field")?;
    dispatch_dirty_input(&mut session)?;
    let (miss_x, miss_y) = blank_pointer_position(&session);
    assert_blank_click_blurs_and_blocks_text(&mut session, miss_x, miss_y)?;
    assert_drag_release_on_blank_keeps_focus(&mut session, miss_x, miss_y)
}

fn blank_blur_session() -> TestResult<super::super::HtmlInteractiveSession> {
    start(
        r#"<input id=field value=initial><p id=status></p><script>
const field = document.getElementById('field');
const status = document.getElementById('status');
for (const type of ['focus', 'keydown', 'keyup', 'input', 'change', 'blur']) {
  field.addEventListener(type, (event) => {
    status.textContent += `${event.type}${event.key ? ':' + event.key : ''}|`;
  });
}
</script>"#,
    )
}

fn dispatch_dirty_input(session: &mut super::super::HtmlInteractiveSession) -> TestResult {
    dispatch_inputs(
        session,
        [
            HtmlBrowserInput::KeyDown {
                key: "A".to_string(),
            },
            HtmlBrowserInput::Text {
                text: "a".to_string(),
            },
            HtmlBrowserInput::KeyUp {
                key: "A".to_string(),
            },
        ],
    )
}

fn assert_blank_click_blurs_and_blocks_text(
    session: &mut super::super::HtmlInteractiveSession,
    miss_x: f32,
    miss_y: f32,
) -> TestResult {
    dispatch_primary_click(session, miss_x, miss_y, miss_x, miss_y)?;
    assert_eq!(session.focused_input, None);
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: " appended".to_string(),
        })
        .map_err(to_string)?;
    assert_eq!(
        input_value(&session.runtime.interactive_nodes().map_err(to_string)?),
        Some("initiala")
    );

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains("focus|keydown:A|input|keyup:A|change|blur|"),
        "{snapshot}"
    );
    Ok(())
}

fn dispatch_committed_input(session: &mut super::super::HtmlInteractiveSession) -> TestResult {
    session
        .dispatch_input(HtmlBrowserInput::KeyDown {
            key: "A".to_string(),
        })
        .map_err(to_string)?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "a".to_string(),
        })
        .map_err(to_string)?;
    session
        .dispatch_input(HtmlBrowserInput::KeyUp {
            key: "A".to_string(),
        })
        .map_err(to_string)?;
    session
        .dispatch_input(HtmlBrowserInput::Focus { focused: false })
        .map_err(to_string)?;
    Ok(())
}

fn blank_pointer_position(session: &super::super::HtmlInteractiveSession) -> (f32, f32) {
    let position = (1..239).find_map(|y| {
        (1..320)
            .find(|x| session.hit_target_at(*x as f32, y as f32).is_none())
            .map(|x| (x as f32, y as f32))
    });
    assert!(
        position.is_some(),
        "document viewport must contain blank space"
    );
    position.unwrap_or((0.0, 0.0))
}

fn assert_drag_release_on_blank_keeps_focus(
    session: &mut super::super::HtmlInteractiveSession,
    miss_x: f32,
    miss_y: f32,
) -> TestResult {
    click_element(session, "field")?;
    let target = session.hit_targets.first().cloned();
    assert!(target.is_some(), "input target must exist");
    let (x, y) = target
        .map(|value| (value.x + 1.0, value.y + 1.0))
        .unwrap_or((0.0, 0.0));
    dispatch_primary_click(session, x, y, miss_x, miss_y)?;
    assert!(session.focused_input.is_some());
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "b".to_string(),
        })
        .map_err(to_string)?;
    assert_eq!(
        input_value(&session.runtime.interactive_nodes().map_err(to_string)?),
        Some("initialab")
    );
    Ok(())
}

#[test]
fn high_density_pointer_coordinates_hit_logical_css_targets() -> TestResult {
    let source = HtmlBrowserSource::new(
        r#"<button id=run style="margin-left:100px" onclick="this.textContent='Clicked'">Run</button>"#,
        "https://example.test/docs/index.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(640, 480, 2.0).map_err(to_string)?;
    let mut session =
        super::super::HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;

    for input in [
        HtmlBrowserInput::PointerDown {
            x: 234.0,
            y: 34.0,
            button: 0,
        },
        HtmlBrowserInput::PointerUp {
            x: 234.0,
            y: 34.0,
            button: 0,
        },
    ] {
        session.dispatch_input(input).map_err(to_string)?;
    }

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains("Clicked"), "{snapshot}");
    Ok(())
}

#[test]
fn relative_link_emits_resolved_navigation() -> TestResult {
    let mut session = start("<a href=guide/next.html>Next</a>")?;
    click_first_target(&mut session)?;
    assert_eq!(
        session
            .take_navigation()
            .map(|navigation| navigation.url.as_str().to_string()),
        Some("https://example.test/docs/guide/next.html".to_string())
    );
    Ok(())
}

#[test]
fn relative_link_navigation_preserves_the_target_fragment() -> TestResult {
    let mut session = start("<a href='linked-panel.html#linked-target'>Next</a>")?;
    click_first_target(&mut session)?;
    assert_eq!(
        session
            .take_navigation()
            .map(|navigation| navigation.url.as_str().to_string()),
        Some("https://example.test/docs/linked-panel.html#linked-target".to_string())
    );
    Ok(())
}

#[test]
fn same_document_fragment_scrolls_without_host_navigation_or_runtime_reset() -> TestResult {
    let mut session = start(&fragment_document())?;
    click_element(&mut session, "mutate")?;
    click_element(&mut session, "jump")?;

    assert!(session.take_navigation().is_none());
    assert_eq!(
        session.source.origin.as_str(),
        "https://example.test/docs/index.html#%74arget"
    );
    assert!(session.scroll_y > 0.0);
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains("changed")
    );
    assert_eq!(
        session.latest_frame().map(|frame| frame.origin.as_str()),
        Some("https://example.test/docs/index.html#%74arget")
    );
    Ok(())
}

#[test]
fn initial_document_fragment_scrolls_before_the_first_public_frame() -> TestResult {
    let source = HtmlBrowserSource::new(
        initial_fragment_document(),
        "https://example.test/docs/index.html#%74arget",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)?;

    let mut session =
        super::super::HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;

    assert!(session.scroll_y > 0.0);
    assert!(session.take_navigation().is_none());
    assert!(
        session
            .latest_frame()
            .is_some_and(|frame| frame_contains_rgb(frame, [232, 199, 255]))
    );
    assert_eq!(
        session.latest_frame().map(|frame| frame.origin.as_str()),
        Some("https://example.test/docs/index.html#%74arget")
    );
    Ok(())
}

#[test]
fn initial_fragment_keeps_top_sticky_sidebar_in_the_viewport() -> TestResult {
    assert_fragment_keeps_sticky_sidebar(STICKY_FRAGMENT_DOCUMENT)
}

#[test]
fn block_layout_keeps_top_sticky_sidebar_in_the_viewport() -> TestResult {
    assert_fragment_keeps_sticky_sidebar(BLOCK_STICKY_FRAGMENT_DOCUMENT)
}

fn assert_fragment_keeps_sticky_sidebar(html: &str) -> TestResult {
    let source = HtmlBrowserSource::new(html, "https://example.test/docs/index.html#s15")
        .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)?;

    let session =
        super::super::HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;

    assert!(session.scroll_y > 0.0);
    let frame = session.latest_frame().ok_or("missing sticky frame")?;
    assert!(
        frame_matching_rgb_pixels(frame, [15, 23, 42]) >= 80 * 200,
        "sticky sidebar disappeared after fragment scroll: scroll_y={}",
        session.scroll_y
    );
    Ok(())
}

#[test]
#[ignore = "requires KATANA_HTML_FIXTURE to point to the supplied desktop HTML"]
fn external_fragment_keeps_sticky_sidebar_in_the_viewport() -> TestResult {
    let path = std::env::var("KATANA_HTML_FIXTURE").map_err(to_string)?;
    let html = std::fs::read_to_string(&path).map_err(to_string)?;
    let mut origin = url::Url::from_file_path(&path)
        .map_err(|()| format!("fixture path cannot be converted to a file URL: {path}"))?;
    origin.set_fragment(Some("s15"));
    let source = HtmlBrowserSource::new(&html, origin.as_str()).map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(1_440, 900, 1.0).map_err(to_string)?;

    let session =
        super::super::HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;

    assert!(session.scroll_y > 0.0);
    let frame = session
        .latest_frame()
        .ok_or("missing external sticky frame")?;
    let sidebar_pixels = frame_matching_rgb_pixels(frame, [15, 23, 42]);
    assert!(
        sidebar_pixels >= 100_000,
        "external sticky sidebar disappeared after fragment scroll: scroll_y={}, sidebar_pixels={sidebar_pixels}",
        session.scroll_y
    );
    Ok(())
}

#[test]
fn initial_document_fragment_reflows_to_the_target_after_viewport_resize() -> TestResult {
    let source = HtmlBrowserSource::new(
        linked_fragment_document(),
        "https://example.test/docs/linked-panel.html#linked-target",
    )
    .map_err(to_string)?;
    let initial_viewport = HtmlBrowserViewport::new(1, 1, 1.0).map_err(to_string)?;
    let mut session =
        super::super::HtmlInteractiveSession::start(source, initial_viewport).map_err(to_string)?;

    session
        .resize(HtmlBrowserViewport::new(320, 1, 2.0).map_err(to_string)?)
        .map_err(to_string)?;
    session
        .resize(HtmlBrowserViewport::new(1946, 1292, 2.0).map_err(to_string)?)
        .map_err(to_string)?;

    let frame = session.latest_frame().ok_or("missing resized frame")?;
    assert!(frame_matching_rgb_pixels(frame, [232, 199, 255]) >= 1_000);
    Ok(())
}

#[test]
fn explicit_scroll_releases_fragment_resize_alignment() -> TestResult {
    let source = HtmlBrowserSource::new(
        initial_fragment_document(),
        "https://example.test/docs/index.html#target",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)?;
    let mut session =
        super::super::HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;

    assert_eq!(session.resize_anchor.as_deref(), Some("target"));
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: -1.0,
        })
        .map_err(to_string)?;

    assert!(session.resize_anchor.is_none());
    Ok(())
}

#[test]
fn host_scroll_is_dispatched_on_window_without_bubbling_to_body() -> TestResult {
    let mut session = start(
        "<p id=host-scroll>initial</p><div style='height: 1000px'></div><script>document.body.addEventListener('scroll', () => document.getElementById('host-scroll').textContent += '-body'); window.addEventListener('scroll', () => document.getElementById('host-scroll').textContent += '-window');</script>",
    )?;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 120.0,
        })
        .map_err(to_string)?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains(">initial-window<"), "{snapshot}");
    assert!(!snapshot.contains("-body"), "{snapshot}");
    Ok(())
}

#[test]
fn clamped_host_scroll_does_not_dispatch_a_window_event() -> TestResult {
    let mut session = start(
        "<p id=host-scroll>initial</p><div style='height: 1000px'></div><script>window.addEventListener('scroll', () => document.getElementById('host-scroll').textContent = 'dispatched');</script>",
    )?;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: -120.0,
        })
        .map_err(to_string)?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains(">initial<"), "{snapshot}");
    Ok(())
}

fn linked_fragment_document() -> String {
    "<!doctype html><html lang=en><head><meta charset=utf-8><style>\
     main { margin: 24px; padding: 24px; border: 2px solid #8e78a9; background: #f4d7ff; }\
     h1 { margin: 0 0 12px; font-size: 30px; color: #17372a; }\
     p { margin: 0 0 12px; } a { color: #0969da; }\
     #linked-target { padding: 16px; border: 2px solid #7d5ba6; background: #e8c7ff; }\
     </style></head><body><main><h1>Linked panel loaded by KRR</h1>\
     <p>KatanA forwarded file navigation to the active KRR HTML session.</p>\
     <div style='height: 900px'></div><section id=linked-target>\
     <h1>Linked fragment target loaded by KRR</h1>\
     <p>The initial KRR frame applied the complete document URL fragment.</p>\
     <a href=index.html>Back to preview</a></section>\
     <div style='height: 400px'></div></main></body></html>"
        .to_string()
}

fn initial_fragment_document() -> String {
    "<style>#target { background: #e8c7ff; height: 120px; }</style>\
     <div style='height: 900px'></div><section id=target>Target</section>\
     <div style='height: 400px'></div>"
        .to_string()
}

#[test]
fn same_document_fragment_removal_scrolls_to_top_without_host_navigation() -> TestResult {
    let mut session = start(&fragment_document())?;
    click_element(&mut session, "jump")?;
    assert!(session.scroll_y > 0.0);

    click_element(&mut session, "top")?;

    assert!(session.take_navigation().is_none());
    assert_eq!(
        session.source.origin.as_str(),
        "https://example.test/docs/index.html"
    );
    assert_eq!(session.scroll_y, 0.0);
    Ok(())
}

#[test]
fn same_url_without_a_fragment_remains_a_main_document_navigation() -> TestResult {
    let mut session = start("<a id=reload href=index.html>Reload</a>")?;
    click_element(&mut session, "reload")?;
    assert_eq!(
        session
            .take_navigation()
            .map(|navigation| navigation.url.as_str().to_string()),
        Some("https://example.test/docs/index.html".to_string())
    );
    Ok(())
}

#[test]
fn missing_fragment_keeps_scroll_and_legacy_named_anchor_is_supported() -> TestResult {
    let mut session = start(&fragment_document())?;
    click_element(&mut session, "jump")?;
    let target_scroll = session.scroll_y;

    click_element(&mut session, "missing-link")?;
    assert_eq!(session.scroll_y, target_scroll);
    assert!(session.take_navigation().is_none());

    click_element(&mut session, "legacy-jump")?;
    assert!(session.scroll_y > 0.0);
    assert_eq!(
        session.source.origin.as_str(),
        "https://example.test/docs/index.html#legacy"
    );
    Ok(())
}

#[test]
fn cross_origin_link_is_rejected_without_a_navigation_event() -> TestResult {
    let mut session = start("<a id=remote href=https://other.example/next.html>Remote</a>")?;
    let error = required_error(click_element(&mut session, "remote"), "cross-origin link")?;

    assert!(error.contains("navigation is not allowed"));
    assert!(session.take_navigation().is_none());
    Ok(())
}

fn fragment_document() -> String {
    format!(
        "<button id=mutate onclick=\"document.getElementById('state').textContent='changed'\">Mutate</button>\
         <p id=state>initial</p><a id=jump href=#%74arget>Jump</a>\
         <a id=top href=index.html>Top</a><a id=missing-link href=#absent>Missing</a>\
         <a id=legacy-jump href=#legacy>Legacy</a>{}\
         <h2 id=target>Target</h2><a name=legacy>Legacy target</a><p>After target</p>",
        "<p>spacer</p>".repeat(30)
    )
}

#[test]
fn summary_click_propagates_click_and_toggle_listener_errors() -> TestResult {
    assert_summary_click_error("click", "summary click error")?;
    assert_summary_click_error("toggle", "summary toggle error")
}

#[test]
fn resize_and_scroll_keep_the_frame_at_the_requested_viewport() -> TestResult {
    let mut session = start(long_document())?;
    let viewport = HtmlBrowserViewport::new(480, 160, 1.0).map_err(to_string)?;
    session.resize(viewport).map_err(to_string)?;
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 80.0,
        })
        .map_err(to_string)?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "resized frame must exist".to_string())?;
    assert_eq!(frame.viewport, viewport);
    assert_eq!(frame.pixels.len(), 480 * 160 * 4);
    Ok(())
}

#[test]
fn passive_invalid_and_unfocused_input_paths_preserve_runtime_ownership() -> TestResult {
    let mut session = start("<input id=entry value=initial><button id=action>Run</button>")?;
    let initial = session
        .latest_frame()
        .map(|frame| frame.generation)
        .ok_or_else(|| "initial frame must exist".to_string())?;
    dispatch_passive_input_events(&mut session)?;
    assert_click_miss_and_mismatch(&mut session)?;
    assert_focus_loss_blocks_text(&mut session, initial)?;
    assert_invalid_pointer_is_rejected(&mut session);
    Ok(())
}

#[test]
fn input_dispatch_surfaces_runtime_errors_without_host_fallbacks() -> TestResult {
    assert_invalid_link_target_is_reported()?;
    assert_unsupported_link_scheme_is_reported()?;
    assert_removed_details_target_is_reported()?;
    assert_timeout_discards_runtime_for_later_input()?;
    Ok(())
}

fn assert_invalid_link_target_is_reported() -> TestResult {
    let mut session = start("<a id=broken href='http://[invalid'>Broken</a>")?;
    let error = required_error(click_element(&mut session, "broken"), "invalid href")?;
    assert!(error.contains("link target is invalid"));
    Ok(())
}

fn assert_unsupported_link_scheme_is_reported() -> TestResult {
    let mut session = start("<a id=mail href=mailto:test@example.test>Mail</a>")?;
    let error = required_error(
        click_element(&mut session, "mail"),
        "unsupported link scheme",
    )?;
    assert!(error.contains("unsupported"));
    Ok(())
}

fn assert_removed_details_target_is_reported() -> TestResult {
    let mut session = start("<details><summary id=toggle>More</summary></details>")?;
    let target = session
        .hit_targets
        .iter_mut()
        .find(|target| matches!(target.kind, HitTargetKind::Summary { .. }))
        .ok_or_else(|| "summary target must exist".to_string())?;
    target.kind = HitTargetKind::Summary {
        details_node_id: u64::MAX,
    };
    let error = required_error(
        click_element(&mut session, "toggle"),
        "stale details target",
    )?;
    assert!(error.contains("does not exist"));
    Ok(())
}

fn assert_timeout_discards_runtime_for_later_input() -> TestResult {
    let mut session =
        start("<input id=entry value=initial><button id=run onclick=\"for (;;) {}\">Run</button>")?;
    click_element(&mut session, "entry")?;
    let timeout = required_error(click_element(&mut session, "run"), "handler timeout")?;
    assert!(timeout.contains("timed out"));
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "after timeout".to_string(),
        })
        .map_err(to_string)?;
    let scroll_error = required_error(
        session
            .dispatch_input(HtmlBrowserInput::Scroll {
                delta_x: NO_HORIZONTAL_SCROLL,
                delta_y: UNIT_SCROLL_DELTA,
            })
            .map_err(to_string),
        "discarded runtime scroll",
    )?;
    assert!(scroll_error.to_string().contains("discarded"));
    Ok(())
}

fn required_error(result: TestResult, subject: &str) -> TestResult<String> {
    match result {
        Ok(()) => Err(format!("{subject} must fail")),
        Err(error) => Ok(error),
    }
}

fn move_pointer_to_element(
    session: &mut super::super::HtmlInteractiveSession,
    id: &str,
) -> TestResult {
    let node_id = session
        .runtime
        .node_for_element_id(id)
        .ok_or_else(|| format!("missing element #{id}"))?
        .0;
    let element = session
        .element_boxes
        .iter()
        .find(|element| element.node_id == node_id)
        .cloned()
        .ok_or_else(|| format!("missing layout box #{id}"))?;
    session
        .dispatch_input(HtmlBrowserInput::PointerMove {
            x: element.x + element.width / 2.0,
            y: element.y + element.height / 2.0 - session.scroll_y,
        })
        .map_err(to_string)
}

fn move_pointer_outside_viewport(session: &mut super::super::HtmlInteractiveSession) -> TestResult {
    session
        .dispatch_input(HtmlBrowserInput::PointerMove { x: -1.0, y: -1.0 })
        .map_err(to_string)
}

fn dispatch_passive_input_events(session: &mut super::super::HtmlInteractiveSession) -> TestResult {
    dispatch_inputs(session, passive_text_events())?;
    dispatch_inputs(session, secondary_pointer_events())?;
    assert_eq!(
        input_value(&session.runtime.interactive_nodes().map_err(to_string)?),
        Some("initial")
    );
    Ok(())
}

fn passive_text_events() -> [HtmlBrowserInput; 5] {
    [
        HtmlBrowserInput::Focus { focused: true },
        HtmlBrowserInput::PointerMove { x: 4.0, y: 4.0 },
        HtmlBrowserInput::KeyDown {
            key: "Tab".to_string(),
        },
        HtmlBrowserInput::KeyUp {
            key: "Tab".to_string(),
        },
        HtmlBrowserInput::Text {
            text: " ignored".to_string(),
        },
    ]
}

fn secondary_pointer_events() -> [HtmlBrowserInput; 2] {
    [
        HtmlBrowserInput::PointerDown {
            x: 4.0,
            y: 4.0,
            button: 1,
        },
        HtmlBrowserInput::PointerUp {
            x: 4.0,
            y: 4.0,
            button: 1,
        },
    ]
}

fn dispatch_inputs<const N: usize>(
    session: &mut super::super::HtmlInteractiveSession,
    inputs: [HtmlBrowserInput; N],
) -> TestResult {
    for input in inputs {
        session.dispatch_input(input).map_err(to_string)?;
    }
    Ok(())
}

fn assert_click_miss_and_mismatch(
    session: &mut super::super::HtmlInteractiveSession,
) -> TestResult {
    dispatch_primary_click(session, 0.0, 0.0, 0.0, 0.0)?;
    let ((target_x, target_y), (other_x, other_y)) = mismatch_target_coordinates(session)?;
    dispatch_primary_click(
        session,
        target_x + 1.0,
        target_y + 1.0,
        other_x + 1.0,
        other_y + 1.0,
    )
}

type TargetCoordinates = (f32, f32);
type MismatchTargetCoordinates = (TargetCoordinates, TargetCoordinates);

fn mismatch_target_coordinates(
    session: &super::super::HtmlInteractiveSession,
) -> TestResult<MismatchTargetCoordinates> {
    let target = session
        .hit_targets
        .first()
        .cloned()
        .ok_or_else(|| "input target must exist".to_string())?;
    let other_target = session
        .hit_targets
        .get(1)
        .cloned()
        .ok_or_else(|| "button target must exist".to_string())?;
    Ok(((target.x, target.y), (other_target.x, other_target.y)))
}

fn dispatch_primary_click(
    session: &mut super::super::HtmlInteractiveSession,
    down_x: f32,
    down_y: f32,
    up_x: f32,
    up_y: f32,
) -> TestResult {
    dispatch_inputs(
        session,
        [
            HtmlBrowserInput::PointerDown {
                x: down_x,
                y: down_y,
                button: 0,
            },
            HtmlBrowserInput::PointerUp {
                x: up_x,
                y: up_y,
                button: 0,
            },
        ],
    )
}

fn assert_focus_loss_blocks_text(
    session: &mut super::super::HtmlInteractiveSession,
    initial: u64,
) -> TestResult {
    click_element(session, "entry")?;
    session
        .dispatch_input(HtmlBrowserInput::Focus { focused: false })
        .map_err(to_string)?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: " ignored again".to_string(),
        })
        .map_err(to_string)?;
    assert_eq!(
        input_value(&session.runtime.interactive_nodes().map_err(to_string)?),
        Some("initial")
    );
    assert!(
        session
            .latest_frame()
            .map(|frame| frame.generation > initial)
            .unwrap_or(false)
    );
    Ok(())
}

fn assert_invalid_pointer_is_rejected(session: &mut super::super::HtmlInteractiveSession) {
    assert!(matches!(
        session.dispatch_input(HtmlBrowserInput::PointerMove {
            x: f32::NAN,
            y: 0.0,
        }),
        Err(crate::renderer::backends::html_browser::HtmlBrowserError::InvalidInputCoordinates)
    ));
}

#[test]
fn orphan_list_item_uses_the_structural_item_layout_path() -> TestResult {
    let session = start(
        "<table></table><article>fallback container</article><br><ul>leading text<li>nested item</li></ul><li>orphan item</li>",
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "list frame must exist".to_string())?;

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

fn assert_summary_click_error(listener: &str, message: &str) -> TestResult {
    let selector = if listener == "click" {
        "summary"
    } else {
        "details"
    };
    let document = format!(
        "<details><summary id=summary>More</summary></details><script>document.querySelector('{selector}').addEventListener('{listener}', () => {{ throw new Error('{message}'); }});</script>"
    );
    let mut session = start(&document)?;
    let error = required_error(click_element(&mut session, "summary"), message)?;

    assert!(error.contains(message));
    Ok(())
}

fn button_document() -> &'static str {
    r#"<button id=run>Run</button><script>document.getElementById('run').addEventListener('click', () => { const button = document.getElementById('run'); button.textContent = 'Done'; button.style.color = '#0a7a2f'; });</script>"#
}

fn frame_generation(
    session: &super::super::HtmlInteractiveSession,
    message: &str,
) -> TestResult<u64> {
    session
        .latest_frame()
        .map(|frame| frame.generation)
        .ok_or_else(|| format!("{message} must exist"))
}

fn event_document() -> &'static str {
    r#"<p id=status data-status>Initial</p><details id=details><summary id=summary>More</summary><input id=entry value=initial><p id=result>Waiting</p></details><script>
const status = document.querySelector('[data-status]');
const details = document.getElementById('details');
const input = document.getElementById('entry');
const result = document.getElementById('result');
document.body.dataset.ready = 'ready';
status.textContent = document.body.dataset.ready;
details.addEventListener('toggle', (event) => { status.textContent = event.currentTarget.open ? 'open' : 'closed'; });
input.addEventListener('input', (event) => { result.textContent = `input:${event.currentTarget.value}`; });
</script>"#
}

fn assert_initial_status(session: &super::super::HtmlInteractiveSession) -> TestResult {
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains("ready")
    );
    Ok(())
}

fn assert_open_status(session: &super::super::HtmlInteractiveSession) -> TestResult {
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains("open")
    );
    Ok(())
}

fn update_event_input(session: &mut super::super::HtmlInteractiveSession) -> TestResult {
    click_element(session, "entry")?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "!".to_string(),
        })
        .map_err(to_string)
}

fn assert_event_input_snapshot(session: &super::super::HtmlInteractiveSession) -> TestResult {
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains("value=\"initial!\""));
    assert!(snapshot.contains("input:initial!"));
    Ok(())
}

fn long_document() -> &'static str {
    "<p>one</p><p>two</p><p>three</p><p>four</p><p>five</p><p>six</p><p>seven</p><p>eight</p><p>nine</p><p>ten</p><p>eleven</p>"
}
