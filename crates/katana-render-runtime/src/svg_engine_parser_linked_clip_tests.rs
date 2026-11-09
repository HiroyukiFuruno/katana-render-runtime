use super::assert_same_visible_pixels;
use rstest::rstest;

const LINKED_CLIP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
  <defs>
    <rect id="wrong-kind" width="4" height="4"/>
    <clipPath id="empty"/>
    <clipPath id="outer" clip-path="url(#TARGET)"><rect x="10" y="10" width="20" height="20"/></clipPath>
  </defs>
  <rect x="10" y="10" width="40" height="20" fill="red" clip-path="url(#outer)"/>
  <rect x="60" y="10" width="10" height="10" fill="blue"/>
</svg>"#;
const HALF_CLIPPED: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
  <rect x="10" y="10" width="20" height="20" fill="red"/>
  <rect x="60" y="10" width="10" height="10" fill="blue"/>
</svg>"#;
const EMPTY_CLIP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80">
  <rect x="60" y="10" width="10" height="10" fill="blue"/>
</svg>"#;

#[rstest]
#[case::wrong_kind("wrong-kind", HALF_CLIPPED)]
#[case::missing("missing", HALF_CLIPPED)]
#[case::empty("empty", EMPTY_CLIP)]
fn linked_clip_distinguishes_invalid_reference_from_empty_geometry(
    #[case] id: &str,
    #[case] oracle: &str,
) -> Result<(), String> {
    assert_same_visible_pixels(&LINKED_CLIP.replace("TARGET", id), oracle)
}

#[test]
fn malformed_clip_transform_matches_upstream_invalid_transform_fixture() -> Result<(), String> {
    let source = include_str!(
        "../../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/masking/clipPath/invalid-transform-on-clipPath.svg"
    );
    let malformed = source.replace("matrix(0 0 0 0 0 0)", "translate(not-a-number)");
    assert_ne!(source, malformed);
    assert_same_visible_pixels(&malformed, source)
}
