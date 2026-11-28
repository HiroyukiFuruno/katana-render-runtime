use super::super::support::{TestResult, click_element, frame_contains_rgb, start};

const INITIAL_COLOR: [u8; 3] = [195, 33, 103];
const UPDATED_COLOR: [u8; 3] = [33, 118, 195];
const COLOR_MUTATION_DOCUMENT: &str = r##"
<div id="host" style="color:#c32167">
  <svg width="160" height="24" viewBox="0 0 160 24">
    <path d="M2 12H158" fill="none" stroke="currentColor" stroke-width="6"/>
  </svg>
</div>
<button id="change">Change</button>
<script>document.getElementById('change').addEventListener('click', () => {
  document.getElementById('host').style.color = '#2176c3';
});</script>
"##;

#[test]
fn inline_svg_current_color_repaints_after_parent_color_changes() -> TestResult {
    let mut session = start(COLOR_MUTATION_DOCUMENT)?;
    let before = session.latest_frame().ok_or("missing initial frame")?;
    let generation = before.generation;
    assert!(frame_contains_rgb(before, INITIAL_COLOR));
    assert!(!frame_contains_rgb(before, UPDATED_COLOR));
    click_element(&mut session, "change")?;
    let after = session.latest_frame().ok_or("missing updated frame")?;
    assert!(after.generation > generation);
    assert!(frame_contains_rgb(after, UPDATED_COLOR));
    assert!(!frame_contains_rgb(after, INITIAL_COLOR));
    Ok(())
}
