use super::support::{TestResult, start_with_viewport, to_string};

#[test]
fn fragmentable_inline_target_includes_atomic_child_geometry_in_real_host() -> TestResult {
    let session = start_with_viewport(ATOMIC_CHILD_DOCUMENT, 80, 100)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    let width = observed_number(&snapshot, "data-atomic-width")?;
    let height = observed_number(&snapshot, "data-atomic-height")?;
    let ratio = observed_number(&snapshot, "data-atomic-ratio")?;

    assert!(
        width >= 28.0 && height >= 18.0,
        "the parent inline rect must include its sized atomic child: {snapshot}"
    );
    assert!(
        ratio > 0.0,
        "the observer must receive the atomic child fragment as an intersecting target: {snapshot}"
    );
    Ok(())
}

const ATOMIC_CHILD_DOCUMENT: &str = r##"<style>
html, body { margin: 0; }
#atomic { display: inline-block; width: 28px; height: 18px; padding: 3px; background: #2c4ac6; }
</style>
<span id=target><i id=atomic></i></span><p id=observed></p>
<script>
const target = document.getElementById("target");
const observed = document.getElementById("observed");
new IntersectionObserver((entries) => {
  const rect = target.getBoundingClientRect();
  observed.setAttribute("data-atomic-width", rect.width);
  observed.setAttribute("data-atomic-height", rect.height);
  observed.setAttribute("data-atomic-ratio", entries[0].intersectionRatio);
}).observe(target);
</script>"##;

fn observed_number(snapshot: &str, name: &str) -> TestResult<f32> {
    let needle = format!(r##"{name}=""##);
    let value = snapshot
        .split(&needle)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .ok_or_else(|| format!("missing {name} in snapshot: {snapshot}"))?;
    value
        .parse::<f32>()
        .map_err(|error| format!("invalid {name} value {value:?}: {error}"))
}
