use skrifa::{FontRef, MetadataProvider, Tag};
use std::collections::BTreeMap;

const ROBOTO_FLEX: &[u8] = include_bytes!(
    "../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/RobotoFlex.subset.ttf"
);
const WEIGHT_AXIS: [u8; 4] = *b"wght";

#[test]
fn variation_coordinates_keep_a_real_explicit_axis_and_other_axis_defaults() -> Result<(), String> {
    let font = fixture_font()?;
    let weight = font
        .axes()
        .get_by_tag(Tag::from_be_bytes(WEIGHT_AXIS))
        .ok_or("Roboto Flex weight axis missing")?;
    let explicit_value = [weight.min_value(), weight.max_value()]
        .into_iter()
        .find(|value| *value != weight.default_value())
        .ok_or("Roboto Flex weight axis has no non-default bound")?;
    assert_ne!(explicit_value, weight.default_value());
    let explicit = BTreeMap::from([(WEIGHT_AXIS, f64::from(explicit_value))]);
    let variations = super::variation_coordinates(&font, &explicit)
        .ok_or("real explicit variation coordinates rejected")?;
    assert_eq!(variations.len(), font.axes().iter().count());
    for (axis, variation) in font.axes().iter().zip(&variations) {
        assert_eq!(variation.tag, axis.tag().to_be_bytes());
        let expected = if variation.tag == WEIGHT_AXIS {
            explicit_value
        } else {
            axis.default_value()
        };
        assert_eq!(variation.value, expected);
    }
    Ok(())
}

#[test]
fn variation_coordinates_reject_unknown_and_out_of_range_real_axes() -> Result<(), String> {
    let font = fixture_font()?;
    let unknown = BTreeMap::from([(*b"zzzz", 1.0)]);
    assert!(super::variation_coordinates(&font, &unknown).is_none());
    let weight = font
        .axes()
        .get_by_tag(Tag::from_be_bytes(WEIGHT_AXIS))
        .ok_or("Roboto Flex weight axis missing")?;
    let out_of_range = BTreeMap::from([(WEIGHT_AXIS, f64::from(weight.max_value()) + 1.0)]);
    assert!(super::variation_coordinates(&font, &out_of_range).is_none());
    Ok(())
}

fn fixture_font() -> Result<FontRef<'static>, String> {
    FontRef::from_index(ROBOTO_FLEX, 0).map_err(|error| error.to_string())
}
