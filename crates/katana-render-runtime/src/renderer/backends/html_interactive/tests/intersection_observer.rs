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
