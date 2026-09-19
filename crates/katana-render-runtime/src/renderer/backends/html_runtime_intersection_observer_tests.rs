use super::{HtmlRenderInput, HtmlRenderer};

type TestResult<T = ()> = Result<T, String>;

#[test]
fn intersection_observer_does_not_invent_visibility_without_layout_geometry() -> TestResult {
    let output = render(
        r##"<a class="toc-item active" href="#one">One</a>
        <a class="toc-item" href="#two">Two</a>
        <section id="one">First</section><section id="two">Second</section>
        <script>
          const sections = document.querySelectorAll('[id]');
          const links = document.querySelectorAll('.toc-item');
          const observer = new IntersectionObserver((entries) => {
            entries.forEach((entry) => {
              if (entry.isIntersecting) entry.target.classList.add('observed');
            });
          });
          links.forEach((link) => link.classList.remove('active'));
          sections.forEach((section) => observer.observe(section));
          observer.disconnect();
        </script>"##,
    )?;

    assert!(!output.contains("toc-item active"), "{output}");
    assert!(!output.contains("observed"), "{output}");
    Ok(())
}

fn render(source: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: source.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render test fixture: {error}"))
}
