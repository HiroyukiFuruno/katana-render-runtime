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
fn fragmentable_inline_line_break_retains_line_height_in_real_host() -> TestResult {
    let session = start_with_viewport(fragmentable_inline_line_break_document(), 80, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    let bounding_width = observed_number(&snapshot, "data-line-break-bounding-width")?;
    let bounding_height = observed_number(&snapshot, "data-line-break-bounding-height")?;

    assert_eq!(
        bounding_width, 0.0,
        "a line break has no inline width: {snapshot}"
    );
    assert!(
        bounding_height > 0.0,
        "a line break must retain its line-height fragment: {snapshot}"
    );
    Ok(())
}

#[test]
fn fragmentable_inline_consecutive_line_breaks_retain_each_line_height_in_real_host() -> TestResult
{
    let session = start_with_viewport(
        fragmentable_inline_consecutive_line_breaks_document(),
        80,
        100,
    )?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    let bounding_width = observed_number(&snapshot, "data-line-breaks-bounding-width")?;
    let bounding_height = observed_number(&snapshot, "data-line-breaks-bounding-height")?;

    assert_eq!(
        bounding_width, 0.0,
        "consecutive line breaks have no inline width: {snapshot}"
    );
    assert_eq!(
        bounding_height, 48.0,
        "each consecutive line break must retain its line-height fragment: {snapshot}"
    );
    Ok(())
}

#[test]
fn positioned_inline_block_is_excluded_from_parent_inline_fragments() -> TestResult {
    let session = start_with_viewport(positioned_inline_block_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    let bounding_width = observed_number(&snapshot, "data-positioned-inline-width")?;

    assert!(
        bounding_width < 200.0,
        "an out-of-flow inline-block must not widen its parent inline target: {snapshot}"
    );
    Ok(())
}

#[test]
fn empty_inline_parent_ignores_out_of_flow_child_fragments() -> TestResult {
    let session = start_with_viewport(
        empty_inline_parent_with_positioned_children_document(),
        160,
        100,
    )?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    let bounding_height = observed_number(&snapshot, "data-empty-positioned-height")?;

    assert_eq!(
        bounding_height, 0.0,
        "out-of-flow children must not inflate an empty inline parent's bounding rect: {snapshot}"
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
fn explicit_root_rejects_static_targets_with_out_of_flow_ancestors() -> TestResult {
    let session = start_with_viewport(out_of_flow_ancestor_targets_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    for target in ["fixed-child", "absolute-child"] {
        assert!(
            snapshot.contains(&format!(r##"data-{target}="false:0""##)),
            "a static target inheriting an out-of-flow ancestor must be excluded from its element root: {snapshot}"
        );
        assert!(
            snapshot.contains(&format!(r##"data-{target}-intersection="0:0""##)),
            "an excluded static target must have an empty intersection: {snapshot}"
        );
    }
    Ok(())
}

#[test]
fn explicit_root_rejects_fragmentable_inline_targets_inside_escaped_positioned_ancestors()
-> TestResult {
    let session = start_with_viewport(
        fragmentable_inline_target_with_escaped_positioned_ancestor_document(),
        160,
        100,
    )?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    for target in ["fixed-inline", "absolute-inline"] {
        assert!(
            snapshot.contains(&format!(r##"data-{target}="false:0""##)),
            "a fragmentable inline target inheriting an escaped positioned ancestor must be excluded from its element root: {snapshot}"
        );
        assert!(
            snapshot.contains(&format!(r##"data-{target}-intersection="0:0""##)),
            "an excluded fragmentable inline target must have an empty intersection: {snapshot}"
        );
    }
    Ok(())
}

#[test]
fn explicit_root_accepts_fixed_target_within_transformed_containing_block() -> TestResult {
    let session = start_with_viewport(
        transformed_containing_block_fixed_target_document(),
        160,
        100,
    )?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-transformed-fixed="true:1""##),
        "a fixed target positioned by a transformed ancestor inside the element root must intersect: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-transformed-fixed-intersection="120:20""##),
        "a transformed containing block must preserve the fixed target intersection geometry: {snapshot}"
    );
    Ok(())
}

#[test]
fn explicit_root_rejects_absolute_target_inside_fixed_ancestor() -> TestResult {
    let session = start_with_viewport(fixed_ancestor_absolute_target_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-fixed-absolute="false:0""##),
        "an absolute target inside a fixed ancestor must remain outside its element root: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-fixed-absolute-intersection="0:0""##),
        "a fixed-ancestor absolute target must have an empty intersection: {snapshot}"
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
fn over_shrunk_root_margin_remains_empty_in_real_host() -> TestResult {
    let session = start_with_viewport(over_shrunk_root_margin_document(), 160, 120)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "a root margin that crosses both root axes must remain non-intersecting: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-intersection="0:0""##),
        "an over-shrunk root margin must produce an empty intersection rectangle: {snapshot}"
    );
    Ok(())
}

#[test]
fn over_shrunk_root_margin_stays_empty_through_an_overflow_ancestor() -> TestResult {
    let session = start_with_viewport(
        over_shrunk_root_margin_with_ancestor_clip_document(),
        160,
        120,
    )?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "an overflow ancestor must not revive a root whose negative margin crossed both axes: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-intersection="0:0""##),
        "a crossed root must keep an empty intersection after ancestor clipping: {snapshot}"
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
    assert!(
        snapshot.contains(r##"data-root-metadata="present""##),
        "an element root must expose intersection metadata for its own root bounds: {snapshot}"
    );
    Ok(())
}

#[test]
fn bordered_overflow_root_uses_padding_edge_for_external_positioned_targets() -> TestResult {
    let session = start_with_viewport(
        bordered_overflow_root_with_external_positioned_targets_document(),
        160,
        100,
    )?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-root-clips-overflow="true""##),
        "an overflow root must serialize its own clip metadata: {snapshot}"
    );
    for target in ["absolute", "fixed"] {
        assert!(
            snapshot.contains(&format!(r##"data-{target}-root-bounds="4:4:80:40""##)),
            "a bordered overflow root must expose its padding edge to an external {target} target: {snapshot}"
        );
    }
    Ok(())
}

#[test]
fn zero_sized_overflow_root_does_not_resurrect_a_distant_target() -> TestResult {
    let session = start_with_viewport(zero_sized_overflow_root_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "a zero-sized overflow root must not make a distant target intersect: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-intersection="0:0""##),
        "a zero-sized overflow root must produce an empty intersection: {snapshot}"
    );
    Ok(())
}

#[test]
fn collapsed_root_margin_does_not_resurrect_a_distant_target() -> TestResult {
    let session = start_with_viewport(collapsed_root_margin_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "a point-sized root margin must not make a distant target intersect: {snapshot}"
    );
    assert!(
        snapshot.contains(r##"data-intersection="0:0""##),
        "a point-sized root margin must produce an empty intersection: {snapshot}"
    );
    Ok(())
}

#[test]
fn element_root_uses_border_box_without_overflow_clip() -> TestResult {
    let session = start_with_viewport(unclipped_bordered_element_root_document(), 160, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="true:1""##),
        "an overflow-visible element root must use its border box for intersection: {snapshot}"
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
fn rotated_rounded_overflow_clip_preserves_corner_geometry_in_real_host() -> TestResult {
    let session = start_with_viewport(rotated_rounded_overflow_clip_document(), 180, 180)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains("data-target=\"true:"),
        "a target inside the rotated rounded clip must intersect: {snapshot}"
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
fn rounded_element_root_margin_keeps_its_corner_excluded_in_real_host() -> TestResult {
    let session = start_with_viewport(rounded_element_root_margin_document(), 160, 120)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "a non-zero root margin must preserve the rounded root corner exclusion: {snapshot}"
    );
    Ok(())
}

#[test]
fn asymmetric_border_keeps_the_uncovered_inner_corner_rounded_in_real_host() -> TestResult {
    let session = start_with_viewport(asymmetric_border_root_document(), 160, 120)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-target="false:0""##),
        "the inner right corner must retain its radius when only the left border is wide: {snapshot}"
    );
    Ok(())
}

#[test]
fn rounded_clip_keeps_corner_radii_when_arc_centers_overlap_in_real_host() -> TestResult {
    let session = start_with_viewport(overlapping_arc_center_document(), 160, 120)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r##"data-bottom-right-start="60:100""##),
        "the bottom-right arc must retain its own radius when its center overlaps the top-right arc: {snapshot}"
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

fn fragmentable_inline_line_break_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#target { line-height: 24px; }
</style>
<span id=target><br></span><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const rect = entries[0].boundingClientRect;
  observed.setAttribute("data-line-break-bounding-width", rect.width);
  observed.setAttribute("data-line-break-bounding-height", rect.height);
}).observe(document.getElementById("target"));
</script>"##
}

fn fragmentable_inline_consecutive_line_breaks_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#target { line-height: 24px; }
</style>
<span id=target><br><br></span><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const rect = entries[0].boundingClientRect;
  observed.setAttribute("data-line-breaks-bounding-width", rect.width);
  observed.setAttribute("data-line-breaks-bounding-height", rect.height);
}).observe(document.getElementById("target"));
</script>"##
}

fn positioned_inline_block_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#target { display: inline; }
#absolute { display: inline-block; position: absolute; left: 200px; top: 0; width: 40px; height: 20px; }
#fixed { display: inline-block; position: fixed; left: 300px; top: 0; width: 40px; height: 20px; }
</style>
<span id=target>before<span id=absolute></span><span id=fixed></span>after</span><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  observed.setAttribute("data-positioned-inline-width", entries[0].boundingClientRect.width);
}).observe(document.getElementById("target"));
</script>"##
}

fn empty_inline_parent_with_positioned_children_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#absolute { position: absolute; left: 0; top: 200px; width: 40px; height: 20px; }
#fixed { position: fixed; left: 40px; top: 200px; width: 40px; height: 20px; }
</style>
<span id=target><span id=absolute></span><span id=fixed></span></span><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  observed.setAttribute("data-empty-positioned-height", entries[0].boundingClientRect.height);
}).observe(document.getElementById("target"));
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

fn out_of_flow_ancestor_targets_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#outer { position: relative; width: 120px; height: 80px; }
#root { width: 120px; height: 80px; }
#fixed-ancestor { position: fixed; top: 0; left: 0; width: 120px; height: 20px; }
#absolute-ancestor { position: absolute; top: 20px; left: 0; width: 120px; height: 20px; }
#fixed-child, #absolute-child { width: 120px; height: 20px; }
</style>
<div id=outer><div id=root><div id=fixed-ancestor><div id=fixed-child>Fixed child</div></div><div id=absolute-ancestor><div id=absolute-child>Absolute child</div></div></div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  for (const entry of entries) {
    observed.setAttribute(`data-${entry.target.id}`, `${entry.isIntersecting}:${entry.intersectionRatio}`);
    observed.setAttribute(`data-${entry.target.id}-intersection`, `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
  }
}, { root }).observe(document.getElementById("fixed-child"));
new IntersectionObserver((entries) => {
  for (const entry of entries) {
    observed.setAttribute(`data-${entry.target.id}`, `${entry.isIntersecting}:${entry.intersectionRatio}`);
    observed.setAttribute(`data-${entry.target.id}-intersection`, `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
  }
}, { root }).observe(document.getElementById("absolute-child"));
</script>"##
}

fn fragmentable_inline_target_with_escaped_positioned_ancestor_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#outer { position: relative; width: 120px; height: 80px; }
#root { width: 120px; height: 80px; }
#fixed-ancestor { position: fixed; top: 0; left: 0; width: 120px; height: 20px; }
#fixed-absolute { position: absolute; top: 0; left: 0; width: 120px; height: 20px; }
#absolute-ancestor { position: absolute; top: 20px; left: 0; width: 120px; height: 20px; }
#fixed-inline, #absolute-inline { display: inline; }
</style>
<div id=outer><div id=root><div id=fixed-ancestor><div id=fixed-absolute><span id=fixed-inline>Fixed inline target</span></div></div><div id=absolute-ancestor><span id=absolute-inline>Absolute inline target</span></div></div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
const observed = document.getElementById("observed");
for (const target of [document.getElementById("fixed-inline"), document.getElementById("absolute-inline")]) {
  new IntersectionObserver((entries) => {
    const entry = entries[0];
    observed.setAttribute(`data-${entry.target.id}`, `${entry.isIntersecting}:${entry.intersectionRatio}`);
    observed.setAttribute(`data-${entry.target.id}-intersection`, `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
  }, { root }).observe(target);
}
</script>"##
}

fn transformed_containing_block_fixed_target_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { width: 120px; height: 80px; }
#transformed { transform: rotate(0deg); width: 120px; height: 20px; }
#transformed-fixed { position: fixed; top: 0; left: 0; width: 120px; height: 20px; }
</style>
<div id=root><div id=transformed><div id=transformed-fixed>Transformed fixed</div></div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  const observed = document.getElementById("observed");
  observed.setAttribute("data-transformed-fixed", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-transformed-fixed-intersection", `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
}, { root }).observe(document.getElementById("transformed-fixed"));
</script>"##
}

fn fixed_ancestor_absolute_target_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { width: 120px; height: 80px; }
#fixed-ancestor { position: fixed; top: 0; left: 0; width: 120px; height: 20px; }
#fixed-absolute { position: absolute; top: 0; left: 0; width: 120px; height: 20px; }
</style>
<div id=root><div id=fixed-ancestor><div id=fixed-absolute>Fixed absolute</div></div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  const observed = document.getElementById("observed");
  observed.setAttribute("data-fixed-absolute", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-fixed-absolute-intersection", `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
}, { root }).observe(document.getElementById("fixed-absolute"));
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

fn over_shrunk_root_margin_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 100px; height: 100px; }
#target { position: absolute; left: 40px; top: 40px; width: 20px; height: 20px; }
</style>
<div id=root><div id=target>Central target</div></div><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  observed.setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-intersection", `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
}, { root: document.getElementById("root"), rootMargin: "-60px" }).observe(document.getElementById("target"));
</script>"##
}

fn over_shrunk_root_margin_with_ancestor_clip_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 100px; height: 100px; }
#clip { width: 100px; height: 100px; overflow: hidden; }
#target { width: 20px; height: 20px; margin: 40px; }
</style>
<div id=root><div id=clip><div id=target>Central target</div></div></div><p id=observed></p>
<script>
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  observed.setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-intersection", `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
}, { root: document.getElementById("root"), rootMargin: "-60px" }).observe(document.getElementById("target"));
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
  const observed = document.getElementById("observed");
  observed.setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-root-metadata", __krrNativeDom("intersectionMetadata", root.__krrNodeId) === "null" ? "missing" : "present");
}, { root }).observe(document.getElementById("target"));
</script>"##
}

fn bordered_overflow_root_with_external_positioned_targets_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#outer { position: relative; width: 120px; height: 80px; }
#root { width: 80px; height: 40px; border: 4px solid; overflow: hidden; }
#absolute { position: absolute; top: 0; left: 0; width: 20px; height: 3px; }
#fixed { position: fixed; top: 0; left: 0; width: 20px; height: 3px; }
</style>
<div id=outer><div id=root><div id=absolute>Absolute</div><div id=fixed>Fixed</div></div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const rootMetadata = JSON.parse(__krrNativeDom("intersectionMetadata", root.__krrNodeId));
  observed.setAttribute("data-root-clips-overflow", String(rootMetadata?.clipsOverflow));
  for (const entry of entries) {
    const bounds = entry.rootBounds;
    observed.setAttribute(`data-${entry.target.id}-root-bounds`, `${bounds.x}:${bounds.y}:${bounds.width}:${bounds.height}`);
  }
}, { root }).observe(document.getElementById("absolute"));
new IntersectionObserver((entries) => {
  for (const entry of entries) {
    const bounds = entry.rootBounds;
    observed.setAttribute(`data-${entry.target.id}-root-bounds`, `${bounds.x}:${bounds.y}:${bounds.width}:${bounds.height}`);
  }
}, { root }).observe(document.getElementById("fixed"));
</script>"##
}

fn zero_sized_overflow_root_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 0; height: 0; overflow: hidden; }
#target { position: absolute; left: 100px; top: 100px; width: 20px; height: 20px; }
</style>
<div id=root><div id=target>Distant target</div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  const observed = document.getElementById("observed");
  observed.setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-intersection", `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
}, { root }).observe(document.getElementById("target"));
</script>"##
}

fn collapsed_root_margin_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 100px; height: 100px; }
#target { position: absolute; left: 100px; top: 100px; width: 20px; height: 20px; }
</style>
<div id=root><div id=target>Distant target</div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  const observed = document.getElementById("observed");
  observed.setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
  observed.setAttribute("data-intersection", `${entry.intersectionRect.width}:${entry.intersectionRect.height}`);
}, { root, rootMargin: "-50px" }).observe(document.getElementById("target"));
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

fn rotated_rounded_overflow_clip_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 160px; height: 160px; }
#clip { position: relative; width: 120px; height: 40px; overflow: hidden; border-radius: 20px; transform: rotate(45deg); }
#target { position: absolute; left: 30px; top: 5px; width: 10px; height: 10px; }
</style>
<div id=root><div id=clip><div id=target>Rounded edge</div></div></div><p id=observed></p>
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

fn rounded_element_root_margin_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 100px; height: 100px; border-radius: 50%; overflow: hidden; }
#target { position: absolute; left: -10px; top: 0; width: 5px; height: 5px; }
</style>
<div id=root><div id=target>Corner</div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  document.getElementById("observed").setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
}, { root, rootMargin: "10px" }).observe(document.getElementById("target"));
</script>"##
}

fn asymmetric_border_root_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 100px; height: 100px; border-radius: 20px; border-left: 20px solid; overflow: hidden; }
#target { position: absolute; left: 115px; top: 0; width: 10px; height: 5px; }
</style>
<div id=root><div id=target>Corner</div></div><p id=observed></p>
<script>
const root = document.getElementById("root");
const target = document.getElementById("target");
new IntersectionObserver((entries) => {
  const entry = entries[0];
  const metadata = JSON.parse(__krrNativeDom("intersectionMetadata", target.__krrNodeId));
  document.getElementById("observed").setAttribute("data-meta", JSON.stringify(metadata));
  document.getElementById("observed").setAttribute("data-poly", JSON.stringify(__krrRoundedClipPolygon(metadata.clips[0])));
  document.getElementById("observed").setAttribute("data-target", `${entry.isIntersecting}:${entry.intersectionRatio}`);
}, { root }).observe(target);
</script>"##
}

fn overlapping_arc_center_document() -> &'static str {
    r##"<style>
html, body { margin: 0; }
#root { position: relative; width: 100px; height: 100px; border-radius: 5px 60px 40px 5px / 5px 60px 40px 5px; overflow: hidden; }
#target { position: absolute; left: 50px; top: 50px; width: 10px; height: 10px; }
</style>
<div id=root><div id=target>Corner</div></div><p id=observed></p>
<script>
const clip = {
  corners: [[0, 0], [100, 0], [100, 100], [0, 100]],
  radii: [[5, 5], [40, 60], [40, 40], [5, 5]],
  radiusX: 0,
  radiusY: 0,
};
const point = __krrRoundedClipPolygon(clip)[17];
document.getElementById("observed").setAttribute("data-bottom-right-start", `${Math.round(point.x)}:${Math.round(point.y)}`);
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
