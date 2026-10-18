use super::CoreTextVariationAxis;
use core_foundation::{base::CFType, dictionary::CFDictionary, number::CFNumber, string::CFString};
use std::collections::BTreeSet;

/* WHY: CoreTextは軸辞書を公開する一方、対応するAppleのCFStringキー定数を非公開にしている。 */
const AXIS_IDENTIFIER_ATTRIBUTE: &str = "NSCTVariationAxisIdentifier";
const AXIS_MINIMUM_ATTRIBUTE: &str = "NSCTVariationAxisMinimumValue";
const AXIS_MAXIMUM_ATTRIBUTE: &str = "NSCTVariationAxisMaximumValue";
const AXIS_DEFAULT_ATTRIBUTE: &str = "NSCTVariationAxisDefaultValue";

pub(super) fn parse_axes(
    axes: &core_foundation::array::CFArray<CFDictionary<CFString, CFType>>,
) -> Result<Vec<CoreTextVariationAxis>, String> {
    let mut values = axes
        .iter()
        .map(|axis| parse_axis(&axis))
        .collect::<Result<Vec<_>, _>>()?;
    values.sort_by_key(|axis| axis.identifier);
    reject_duplicate_ids(values.iter().map(|axis| axis.identifier))?;
    Ok(values)
}

fn parse_axis(axis: &CFDictionary<CFString, CFType>) -> Result<CoreTextVariationAxis, String> {
    let identifier = number_attribute(axis, AXIS_IDENTIFIER_ATTRIBUTE)?;
    let identifier = number_to_identifier(&identifier)?;
    let minimum_value = axis_value(axis, AXIS_MINIMUM_ATTRIBUTE, "minimum")?;
    let maximum_value = axis_value(axis, AXIS_MAXIMUM_ATTRIBUTE, "maximum")?;
    let default_value = axis_value(axis, AXIS_DEFAULT_ATTRIBUTE, "default")?;
    validate_axis_bounds(minimum_value, maximum_value, default_value)?;
    Ok(CoreTextVariationAxis {
        identifier,
        minimum_value,
        maximum_value,
        default_value,
    })
}

fn axis_value(
    axis: &CFDictionary<CFString, CFType>,
    name: &str,
    display_name: &str,
) -> Result<f64, String> {
    number_attribute(axis, name)?
        .to_f64()
        .ok_or_else(|| format!("CoreText returned an invalid variation-axis {display_name}"))
}

fn validate_axis_bounds(minimum: f64, maximum: f64, default: f64) -> Result<(), String> {
    if !minimum.is_finite()
        || !maximum.is_finite()
        || !default.is_finite()
        || minimum > maximum
        || default < minimum
        || default > maximum
    {
        return Err("CoreText returned invalid variation-axis bounds".to_string());
    }
    Ok(())
}

pub(super) fn number_to_identifier(number: &CFNumber) -> Result<u32, String> {
    let integer = number
        .to_i64()
        .ok_or_else(|| "CoreText returned an invalid variation identifier".to_string())?;
    let identifier = u32::try_from(integer)
        .map_err(|_| "CoreText returned an out-of-range variation identifier".to_string())?;
    let exact = number
        .to_f64()
        .is_some_and(|value| value.is_finite() && value == f64::from(identifier));
    if !exact {
        return Err("CoreText variation identifier is not an exact u32".to_string());
    }
    Ok(identifier)
}

fn number_attribute(
    attributes: &CFDictionary<CFString, CFType>,
    name: &str,
) -> Result<CFNumber, String> {
    attributes
        .find(CFString::new(name))
        .and_then(|value| value.downcast::<CFNumber>())
        .ok_or_else(|| format!("CoreText variation axis is missing numeric {name}"))
}

pub(super) fn reject_duplicate_ids(mut ids: impl Iterator<Item = u32>) -> Result<(), String> {
    let mut unique = BTreeSet::new();
    if ids.any(|identifier| !unique.insert(identifier)) {
        return Err("CoreText returned duplicate variation identifiers".to_string());
    }
    Ok(())
}
