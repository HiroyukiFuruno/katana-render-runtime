use super::support::{TestResult, start_with_viewport, to_string};
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
fn hidden_and_detached_observer_targets_transition_after_real_host_reflow() -> TestResult {
    let mut session = start_with_viewport(hidden_and_detached_document(), 160, 100)?;

    assert_observer_states(&session, "true:1", "true:1")?;

    dispatch_scroll(&mut session)?;
    dispatch_scroll(&mut session)?;

    assert_observer_states(&session, "false:0", "false:0")
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
