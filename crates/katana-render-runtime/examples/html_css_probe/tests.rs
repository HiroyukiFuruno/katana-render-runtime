use super::{open_session, save_slides};
use std::fs;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const FIXTURE: &str = r#"<!doctype html><html><body style="margin:0;background:rgb(12,34,56)">
<script>
document.addEventListener('keydown', event => {
  if (event.key === 'ArrowRight') document.body.style.background = 'rgb(65,43,21)';
});
</script></body></html>"#;

#[test]
fn save_slides_writes_css_frames_before_and_after_keyboard_navigation() -> TestResult {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("fixture.html");
    let output = directory.path().join("frames");
    fs::write(&source, FIXTURE)?;
    fs::create_dir(&output)?;

    let mut session = open_session(&source)?;
    let slides = [
        ("krr-slide-01.png", [12, 34, 56, 255]),
        ("krr-slide-02.png", [65, 43, 21, 255]),
    ];
    save_slides(&mut session, &output, slides.len())?;
    for (path, color) in slides {
        let frame = image::open(output.join(path))?.into_rgba8();
        assert_eq!(frame.dimensions(), (1230, 867));
        assert_eq!(frame.get_pixel(615, 433).0, color);
    }
    Ok(())
}
