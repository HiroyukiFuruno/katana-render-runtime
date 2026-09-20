use super::support::{TestResult, click_element, start_with_viewport, to_string};
use crate::renderer::backends::html_browser::HtmlBrowserInput;

#[test]
fn intersection_observer_tracks_rendered_sections_after_real_host_scroll() -> TestResult {
    let mut session = start_with_viewport(intersection_document(), 160, 100)?;

    assert_active_anchor(&session, "one")?;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 160.0,
        })
        .map_err(to_string)?;

    assert_active_anchor(&session, "two")?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains(r##"<p id="scroll-observed">#two</p>"##),
        "scroll listener must observe the geometry-derived active section: {snapshot}"
    );
    Ok(())
}

#[test]
fn negative_bottom_root_margin_changes_toc_during_real_host_scroll() -> TestResult {
    let mut session = start_with_viewport(root_margin_document(), 160, 100)?;

    assert_active_anchor(&session, "one")?;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 80.0,
        })
        .map_err(to_string)?;

    assert_active_anchor(&session, "two")
}

#[test]
fn fixed_observer_geometry_is_current_when_the_scroll_handler_runs() -> TestResult {
    let mut session = start_with_viewport(fixed_observer_document(), 160, 100)?;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 120.0,
        })
        .map_err(to_string)?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains(r##"<p id="scroll-observed">visible</p>"##),
        "the scroll handler must receive the fixed element's reflowed geometry: {snapshot}"
    );
    Ok(())
}

#[test]
fn observer_dom_mutation_is_reflowed_before_the_scroll_handler_runs() -> TestResult {
    let mut session = start_with_viewport(observer_mutation_document(), 160, 100)?;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 80.0,
        })
        .map_err(to_string)?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains(r##"<p id="scroll-observed">current</p>"##),
        "the scroll handler must run after observer-triggered reflow: {snapshot}"
    );
    Ok(())
}

#[test]
fn observer_content_collapse_clamps_scroll_before_the_scroll_handler_runs() -> TestResult {
    let mut session = start_with_viewport(observer_collapse_document(), 160, 100)?;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 400.0,
        })
        .map_err(to_string)?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    let expected_scroll = session.max_scroll();
    assert_eq!(session.scroll_y, expected_scroll);
    assert!(
        snapshot.contains(&format!(r##"data-scroll-y="{expected_scroll}""##)),
        "the scroll handler must receive the collapsed layout's max scroll {expected_scroll}: {snapshot}"
    );
    Ok(())
}

#[test]
fn post_event_observers_stabilize_two_reflow_rounds_before_the_frame_is_exposed() -> TestResult {
    let mut session = start_with_viewport(post_event_observer_document(), 160, 100)?;

    click_element(&mut session, "trigger")?;

    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains(r##"id="second" data-observer-stage="second""##),
        "the second observer must see layout produced by the click-triggered first observer: {snapshot}"
    );
    assert!(
        !snapshot.contains("data-observer-stage=\"third\""),
        "the host must bound one event's observer reflow work to two rounds: {snapshot}"
    );
    Ok(())
}

#[test]
fn hidden_and_detached_observer_targets_transition_after_real_host_reflow() -> TestResult {
    let mut session = start_with_viewport(hidden_and_detached_document(), 160, 100)?;

    assert_observer_states(&session, "true:1", "true:1")?;

    dispatch_scroll(&mut session)?;
    dispatch_scroll(&mut session)?;

    assert_observer_states(&session, "false:0", "false:0")
}

#[test]
fn fragmentable_inline_target_uses_union_area_and_ancestor_clip_in_real_host() -> TestResult {
    let session = start_with_viewport(fragmentable_inline_document(), 80, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    let ratio = observed_number(&snapshot, "data-fragment-ratio")?;
    let intersection_height = observed_number(&snapshot, "data-fragment-height")?;
    let intersection_width = observed_number(&snapshot, "data-fragment-width")?;
    let bounding_height = observed_number(&snapshot, "data-fragment-bounding-height")?;
    let bounding_width = observed_number(&snapshot, "data-fragment-bounding-width")?;

    assert!(
        ratio > 0.0 && ratio < 1.0,
        "the clipped first inline fragment must be a strict subset of the target union: {snapshot}"
    );
    assert!(
        intersection_height > 0.0 && intersection_height < 40.0,
        "the intersection must use the visible fragment, not the target's union bounding box: {snapshot}"
    );
    let expected_ratio =
        intersection_width * intersection_height / (bounding_width * bounding_height);
    assert!(
        (ratio - expected_ratio).abs() < 0.0001,
        "the ratio denominator must be the target boundingClientRect area: expected {expected_ratio}, got {ratio}: {snapshot}"
    );
    Ok(())
}

#[test]
fn clipped_wrapped_inline_uses_its_full_bounding_rect_in_real_host() -> TestResult {
    let session = start_with_viewport(clipped_wrapped_inline_document(), 80, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    let intersection_width = observed_number(&snapshot, "data-clipped-intersection-width")?;
    let bounding_width = observed_number(&snapshot, "data-clipped-bounding-width")?;
    let ratio = observed_number(&snapshot, "data-clipped-ratio")?;

    assert!(
        (intersection_width - bounding_width).abs() < 0.0001,
        "clipping a wide line must retain the target bounding width when only a short wrapped line remains visible: {snapshot}"
    );
    assert!(
        ratio > 0.0 && ratio < 1.0,
        "the clipped target must retain a partial intersection ratio: {snapshot}"
    );
    Ok(())
}

#[test]
fn fully_visible_wrapped_inline_target_reaches_threshold_one_in_real_host() -> TestResult {
    let session = start_with_viewport(fully_visible_wrapped_inline_document(), 80, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-full-fragment-ratio="1""##),
        "a fully visible wrapped inline target must report ratio one: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-full-fragment-threshold="true""##),
        "a fully visible wrapped inline target must meet threshold one: {snapshot}"
    );
    Ok(())
}

#[test]
fn empty_inline_target_uses_its_insertion_position() -> TestResult {
    let session = start_with_viewport(empty_inline_document(), 100, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-empty="false:0"##),
        "an empty inline target below the viewport must not intersect at the viewport origin: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-empty-rect="0:120:0:0"##),
        "an empty inline target must retain its insertion position: {snapshot}"
    );
    Ok(())
}

#[test]
fn edge_adjacent_target_keeps_its_intersection_coordinates() -> TestResult {
    let session = start_with_viewport(edge_adjacent_document(), 100, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-edge="true:0""##),
        "an edge-adjacent target must intersect at ratio zero: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-edge-rect="100:0:0:20""##),
        "an edge-adjacent intersection must retain its zero-width edge coordinates: {snapshot}"
    );
    Ok(())
}

#[test]
fn explicit_root_rejects_overlapping_siblings_and_its_own_target() -> TestResult {
    let session = start_with_viewport(explicit_root_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    for (target, expected) in [
        ("child", "true:1"),
        ("outside", "false:0"),
        ("root", "false:0"),
    ] {
        assert!(
            snapshot.contains(&format!(r##"data-{target}="{expected}""##)),
            "expected explicit-root target #{target} to be {expected}: {snapshot}"
        );
    }
    for target in ["outside", "root"] {
        assert!(
            snapshot.contains(&format!(r##"data-{target}-intersection="0:0""##)),
            "expected explicit-root target #{target} to have an empty intersection: {snapshot}"
        );
    }
    Ok(())
}

#[test]
fn explicit_root_rejects_absolute_targets_positioned_by_an_outer_ancestor() -> TestResult {
    let session = start_with_viewport(
        absolute_target_with_outer_containing_block_document(),
        160,
        100,
    )?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-escaped="false:0""##),
        "an absolute target positioned by an ancestor outside the element root must be excluded: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-escaped-intersection="0:0""##),
        "an excluded absolute target must have an empty intersection: {snapshot}"
    );
    Ok(())
}

#[test]
fn element_root_margin_is_not_clipped_by_the_root_overflow() -> TestResult {
    let session = start_with_viewport(root_margin_with_overflow_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-margin-target="true:1""##),
        "the element root's own overflow clip must not trim its expanded root margin: {snapshot}"
    );
    Ok(())
}

#[test]
fn bordered_element_root_uses_its_padding_edge_for_intersection() -> TestResult {
    let session = start_with_viewport(bordered_element_root_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "a target positioned in the root border must not intersect its padding-edge root: {snapshot}"
    );
    Ok(())
}

#[test]
fn element_root_uses_padding_edge_without_overflow_clip() -> TestResult {
    let session = start_with_viewport(unclipped_bordered_element_root_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "a target in the root border must not intersect even without overflow clipping: {snapshot}"
    );
    Ok(())
}

#[test]
fn rotated_overflow_clip_keeps_its_shape_in_real_host() -> TestResult {
    let session = start_with_viewport(rotated_overflow_clip_document(), 180, 180)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "a target in the rotated clip AABB but outside its shape must not intersect: {snapshot}"
    );
    Ok(())
}

#[test]
fn rounded_element_root_clip_excludes_its_corner_in_real_host() -> TestResult {
    let session = start_with_viewport(rounded_element_root_document(), 160, 120)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "a target entirely inside the rectangular corner but outside the rounded root clip must not intersect: {snapshot}"
    );
    Ok(())
}

#[test]
fn separated_ancestor_clip_does_not_revive_edge_contact() -> TestResult {
    let session = start_with_viewport(separated_ancestor_clip_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "an ancestor clip separated from the root must remain non-intersecting: {snapshot}"
    );
    Ok(())
}

#[test]
fn document_root_uses_viewport_geometry() -> TestResult {
    let session = start_with_viewport(document_root_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains(r##"data-document-target="true:1""##),
        "document root must observe in-viewport targets: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-root-bounds="160:100""##),
        "document root bounds must use the viewport: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-document-intersection="160:20""##),
        "document root intersection must retain the target geometry: {snapshot}"
    );
    Ok(())
}

fn intersection_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
section { height: 120px; }
#one { background: #ff0000; }
#two { background: #00ff00; }
</style>
<nav><a class=toc-item href=#one>One</a><a class=toc-item href=#two>Two</a></nav>
<main><section id=one>First</section><section id=two>Second</section><div style="height: 120px"></div></main>
<p id=scroll-observed></p>
<script>
const observer = new IntersectionObserver((entries) => {
  for (const entry of entries) {
    document.querySelector(`a[href="#${entry.target.id}"]`).classList.toggle("active", entry.isIntersecting);
  }
});
document.querySelectorAll("section").forEach((section) => observer.observe(section));
window.addEventListener("scroll", () => {
  document.getElementById("scroll-observed").textContent = document.querySelector(".toc-item.active").getAttribute("href");
});
</script>"##
}

fn root_margin_document() -> &'static str {
    r##"<style>html, body { margin: 0; } nav { position: absolute; } section { height: 80px; }</style>
<nav><a class=toc-item href=#one>One</a><a class=toc-item href=#two>Two</a></nav>
<main><section id=one>First</section><section id=two>Second</section><div style="height: 120px"></div></main>
<script>
const observer = new IntersectionObserver((entries) => {
  for (const entry of entries) {
    document.querySelector(`a[href="#${entry.target.id}"]`).classList.toggle("active", entry.isIntersecting);
  }
}, { rootMargin: "0px 0px -80px 0px" });
document.querySelectorAll("section").forEach((section) => observer.observe(section));
</script>"##
}

fn fixed_observer_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#fixed { position: fixed; top: 0; width: 160px; height: 20px; }
#spacer { height: 400px; }
</style>
<div id=fixed>Fixed</div><div id=spacer></div><p id=intersection></p><p id=scroll-observed></p>
<script>
const observer = new IntersectionObserver((entries) => {
  for (const entry of entries) {
    document.getElementById("intersection").textContent = entry.isIntersecting ? "visible" : "hidden";
  }
});
observer.observe(document.getElementById("fixed"));
window.addEventListener("scroll", () => {
  document.getElementById("scroll-observed").textContent = document.getElementById("intersection").textContent;
});
</script>"##
}

fn observer_mutation_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#before { height: 120px; }
#trigger { height: 20px; }
#changed { display: none; height: 20px; }
#after { height: 300px; }
</style>
<div id=before></div><div id=trigger>Trigger</div><div id=changed>Changed</div><div id=after></div>
<p id=geometry>initial</p><p id=scroll-observed>initial</p>
<script>
const changed = document.getElementById("changed");
new IntersectionObserver((entries) => {
  if (entries.some((entry) => entry.isIntersecting)) changed.style.display = "block";
}).observe(document.getElementById("trigger"));
new IntersectionObserver((entries) => {
  if (entries.some((entry) => entry.isIntersecting)) document.getElementById("geometry").textContent = "current";
}).observe(changed);
window.addEventListener("scroll", () => {
  document.getElementById("scroll-observed").textContent = document.getElementById("geometry").textContent;
});
</script>"##
}

fn observer_collapse_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#collapse { height: 400px; }
#trigger { height: 20px; }
#after { height: 100px; }
</style>
<div id=collapse></div><div id=trigger>Trigger</div><div id=after></div>
<p id=scroll-observed>initial</p>
<script>
new IntersectionObserver((entries) => {
  if (entries.some((entry) => entry.isIntersecting)) {
    document.getElementById("collapse").style.display = "none";
  }
}).observe(document.getElementById("trigger"));
window.addEventListener("scroll", () => {
  document.getElementById("scroll-observed").setAttribute("data-scroll-y", window.scrollY);
});
</script>"##
}

fn hidden_and_detached_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#hidden, #removed { height: 20px; }
#spacer { height: 400px; }
</style>
<div id=hidden>Hidden</div><div id=removed>Removed</div><div id=spacer></div><p id=observed></p>
<script>
const observed = document.getElementById("observed");
const hidden = document.getElementById("hidden");
const removed = document.getElementById("removed");
const observer = new IntersectionObserver((entries) => {
  for (const entry of entries) {
    observed.setAttribute(`data-${entry.target.id}`, `${entry.isIntersecting}:${entry.intersectionRatio}`);
  }
});
observer.observe(hidden);
observer.observe(removed);
window.addEventListener("scroll", () => {
  hidden.style.display = "none";
  removed.remove();
});
</script>"##
}

fn fragmentable_inline_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { width: 80px; height: 20px; overflow: hidden; }
</style>
<div id=root><span id=target>one two three four five six seven eight</span></div><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  observed.setAttribute("data-fragment-ratio", entry.intersectionRatio);
  observed.setAttribute("data-fragment-height", entry.intersectionRect.height);
  observed.setAttribute("data-fragment-width", entry.intersectionRect.width);
  observed.setAttribute("data-fragment-bounding-height", entry.boundingClientRect.height);
  observed.setAttribute("data-fragment-bounding-width", entry.boundingClientRect.width);
}, { root: document.getElementById("root") }).observe(document.getElementById("target"));
</script>"##
}

fn clipped_wrapped_inline_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { width: 80px; height: 20px; overflow: hidden; }
#target { position: relative; top: -20px; }
</style>
<div id=root><span id=target>one two three four five six seven eight</span></div><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  observed.setAttribute("data-clipped-intersection-width", entry.intersectionRect.width);
  observed.setAttribute("data-clipped-bounding-width", entry.boundingClientRect.width);
  observed.setAttribute("data-clipped-ratio", entry.intersectionRatio);
}, { root: document.getElementById("root") }).observe(document.getElementById("target"));
</script>"##
}

fn fully_visible_wrapped_inline_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { width: 80px; height: 200px; overflow: hidden; }
</style>
<div id=root><span id=target>one two three four five six seven eight</span></div><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  observed.setAttribute("data-full-fragment-ratio", entry.intersectionRatio);
  observed.setAttribute("data-full-fragment-threshold", entry.intersectionRatio >= 1);
}, { root: document.getElementById("root"), threshold: 1 }).observe(document.getElementById("target"));
</script>"##
}

fn empty_inline_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#spacer { height: 120px; }
</style>
<div id=spacer></div><span id=empty></span><p id=observed></p>
<script>
new IntersectionObserver((entries) => {
  const entry = entries[0];
  const observed = document.getElementById("observed");
  observed.setAttribute("data-empty", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-empty-rect", `${entry.boundingClientRect.x}:${entry.boundingClientRect.y}:${entry.boundingClientRect.width}:${entry.boundingClientRect.height}`);
}).observe(document.getElementById("empty"));
</script>"##
}

fn edge_adjacent_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#edge { position: absolute; left: 100px; top: 0; width: 20px; height: 20px; }
</style>
<div id=edge>Edge</div><p id=observed></p>
<script>
new IntersectionObserver((entries) => {
  const entry = entries[0];
  const observed = document.getElementById("observed");
  observed.setAttribute("data-edge", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-edge-rect", `${entry.intersectionRect.x}:${entry.intersectionRect.y}:${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
}).observe(document.getElementById("edge"));
</script>"##
}

fn explicit_root_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: absolute; top: 0; left: 0; width: 120px; height: 80px; }
#child, #outside { position: absolute; top: 0; left: 0; width: 120px; height: 80px; }
</style>
<div id=root><div id=child>Child</div></div><div id=outside>Outside</div><p id=observed></p>
<script>
const root = document.getElementById("root");
const observed = document.getElementById("observed");
const observer = new IntersectionObserver((entries) => {
  for (const entry of entries) {
    observed.setAttribute(`data-${entry.target.id}`, `${entry.isIntersecting}:${entry.intersectionRatio}`);
    observed.setAttribute(`data-${entry.target.id}-intersection`, `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
  }
}, { root });
for (const target of [document.getElementById("child"), document.getElementById("outside"), root]) {
  observer.observe(target);
}
</script>"##
}

fn absolute_target_with_outer_containing_block_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#outer { position: relative; width: 120px; height: 80px; }
#root { width: 120px; height: 80px; }
#escaped { position: absolute; top: 0; left: 0; width: 120px; height: 80px; }
</style>
<div id=outer><div id=root><div id=escaped>Escaped</div></div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  document.getElementById("observed").setAttribute("data-escaped", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  document.getElementById("observed").setAttribute("data-escaped-intersection", `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
}, { root }).observe(document.getElementById("escaped"));
</script>"##
}

fn root_margin_with_overflow_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 80px; height: 20px; overflow: hidden; }
#margin-target { position: absolute; top: -5px; left: 0; width: 80px; height: 10px; }
</style>
<div id=root><div id=margin-target>Margin target</div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  document.getElementById("observed").setAttribute("data-margin-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
}, { root, rootMargin: "10px" }).observe(document.getElementById("margin-target"));
</script>"##
}

fn bordered_element_root_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 80px; height: 40px; border: 4px solid; overflow: hidden; }
#target { position: absolute; top: 0; left: 4px; width: 20px; height: 3px; }
</style>
<div id=root><div id=target>Border</div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  document.getElementById("observed").setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
}, { root }).observe(document.getElementById("target"));
</script>"##
}

fn unclipped_bordered_element_root_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 80px; height: 40px; border: 4px solid; }
#target { position: absolute; top: 0; left: 4px; width: 20px; height: 3px; }
</style>
<div id=root><div id=target>Border</div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  document.getElementById("observed").setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
}, { root }).observe(document.getElementById("target"));
</script>"##
}

fn rotated_overflow_clip_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 160px; height: 160px; }
#clip { position: relative; width: 100px; height: 100px; overflow: hidden; transform: rotate(45deg); }
#target { position: absolute; left: 120px; top: 0; width: 20px; height: 20px; }
</style>
<div id=root><div id=clip><div id=target>Outside</div></div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  document.getElementById("observed").setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
}, { root }).observe(document.getElementById("target"));
</script>"##
}

fn rounded_element_root_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 100px; height: 100px; border-radius: 50%; overflow: hidden; }
#target { position: absolute; left: 0; top: 0; width: 10px; height: 10px; }
</style>
<div id=root><div id=target>Corner</div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  document.getElementById("observed").setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
}, { root }).observe(document.getElementById("target"));
</script>"##
}

fn separated_ancestor_clip_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 100px; height: 100px; overflow: hidden; }
#outer-clip { position: absolute; left: 200px; top: 0; width: 100px; height: 100px; overflow: hidden; }
#inner-clip { width: 100px; height: 100px; overflow: hidden; }
#target { width: 100px; height: 20px; }
</style>
<div id=root><div id=outer-clip><div id=inner-clip><div id=target>Target</div></div></div></div><p id=observed></p>
<script>
new IntersectionObserver((entries) => {
  const entry = entries[0];
  document.getElementById("observed").setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
}, { root: document.getElementById("root") }).observe(document.getElementById("target"));
</script>"##
}

fn document_root_document() -> &'static str {
    r##"<style>html, body { margin: 0; } #document-target { height: 20px; }</style>
<div id=document-target>Document target</div><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  observed.setAttribute("data-document-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-root-bounds", `${entry.rootBounds.width}:${entry.rootBounds.height}`);
  observed.setAttribute("data-document-intersection", `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
}, { root: document }).observe(document.getElementById("document-target"));
</script>"##
}

fn observed_number(snapshot: &str, attribute: &str) -> TestResult<f32> {
    let prefix = format!("{attribute}=\"");
    let value = snapshot
        .split(&prefix)
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .ok_or_else(|| format!("missing {attribute}: {snapshot}"))?;
    value
        .parse::<f32>()
        .map_err(|error| format!("invalid {attribute} {value:?}: {error}"))
}

fn dispatch_scroll(session: &mut super::super::HtmlInteractiveSession) -> TestResult {
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 10.0,
        })
        .map_err(to_string)
}

fn assert_observer_states(
    session: &super::super::HtmlInteractiveSession,
    hidden: &str,
    removed: &str,
) -> TestResult {
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains(&format!(r##"data-hidden="{hidden}""##)),
        "expected hidden target to be {hidden}: {snapshot}"
    );
    assert!(
        snapshot.contains(&format!(r##"data-removed="{removed}""##)),
        "expected removed target to be {removed}: {snapshot}"
    );
    Ok(())
}

fn assert_active_anchor(
    session: &super::super::HtmlInteractiveSession,
    active: &str,
) -> TestResult {
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(
        snapshot.contains(&format!(r##"class="toc-item active" href="#{active}""##)),
        "expected #{active} to be active: {snapshot}"
    );
    Ok(())
}

fn post_event_observer_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#first, #second { display: none; height: 20px; }
</style>
<button id=trigger>Trigger</button><div id=first>First</div><div id=second>Second</div>
<script>
const first = document.getElementById("first");
const second = document.getElementById("second");
const third = new IntersectionObserver((entries) => {
  if (entries.some((entry) => entry.isIntersecting)) second.setAttribute("data-observer-stage", "third");
});
const secondObserver = new IntersectionObserver((entries) => {
  if (entries.some((entry) => entry.isIntersecting)) {
    second.setAttribute("data-observer-stage", "second");
    third.observe(second);
  }
});
const firstObserver = new IntersectionObserver((entries) => {
  if (entries.some((entry) => entry.isIntersecting)) {
    first.style.display = "block";
    secondObserver.observe(first);
  }
});
document.getElementById("trigger").addEventListener("click", () => firstObserver.observe(document.getElementById("trigger")));
</script>"##
}
