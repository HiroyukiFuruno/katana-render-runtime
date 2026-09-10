use super::{HtmlRenderInput, HtmlRenderer};

type TestResult<T = ()> = Result<T, String>;

#[test]
fn intersection_observer_accepts_document_collections_and_class_list_updates() -> TestResult {
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
    assert!(output.contains("observed"), "{output}");
    Ok(())
}

#[test]
fn intersection_observer_updates_only_the_anchor_for_scroll_changes() -> TestResult {
    let output = render(scroll_anchor_fixture())?;

    assert_scroll_anchor_state(&output);
    Ok(())
}

fn scroll_anchor_fixture() -> &'static str {
    r##"<nav>
          <a class="toc-item" href="#one">One</a>
          <a class="toc-item" href="#two">Two</a>
        </nav>
        <main>
          <section id="one" data-krr-intersecting="true">First</section>
          <section id="two" data-krr-intersecting="false">Second</section>
        </main>
        <p id="events"></p>
        <script>
          const links = document.querySelectorAll('.toc-item');
          const sections = document.querySelectorAll('section');
          const events = document.getElementById('events');
          const observer = new IntersectionObserver((entries) => {
            events.textContent += entries.map((entry) => `${entry.target.id}:${entry.isIntersecting}`).join(',') + '|';
            entries.forEach((entry) => {
              const link = document.querySelector(`a[href="#${entry.target.id}"]`);
              link.classList.toggle('active', entry.isIntersecting);
            });
          });
          sections.forEach((section) => observer.observe(section));
          sections[0].setAttribute('data-krr-intersecting', 'false');
          sections[1].setAttribute('data-krr-intersecting', 'true');
          window.dispatchEvent(new Event('scroll'));
        </script>"##
}

fn assert_scroll_anchor_state(output: &str) {
    assert!(
        output.contains(r##"<a class="toc-item" href="#one">One</a>"##),
        "{output}"
    );
    assert!(
        output.contains(r##"<a class="toc-item active" href="#two">Two</a>"##),
        "{output}"
    );
    assert!(
        output.contains("one:true|two:false|one:false,two:true|"),
        "{output}"
    );
}

fn render(source: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: source.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render test fixture: {error}"))
}
