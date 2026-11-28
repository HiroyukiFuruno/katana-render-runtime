use super::super::super::{
    CoreTextDescriptorEvidence, CoreTextVariationAxis, CoreTextVariationCoordinate,
};
use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use core_text::{
    font,
    font_descriptor::{
        CTFontDescriptor, CTFontDescriptorCopyAttribute, kCTFontNameAttribute,
        kCTFontVariationAttribute,
    },
};
use std::fs;

#[path = "identity_coretext_axes.rs"]
mod axis_values;

const WEIGHT_AXIS: u32 = u32::from_be_bytes(*b"wght");
const POSTSCRIPT_CHECK_SIZE: f64 = 12.0;

#[cfg(test)]
#[path = "identity_coretext_variation_tests.rs"]
mod tests;

pub(super) fn verify_ctfont_descriptor_name(
    descriptor: &CTFontDescriptor,
    descriptor_postscript_name: &str,
) -> Result<(), String> {
    let font = font::new_from_descriptor(descriptor, POSTSCRIPT_CHECK_SIZE);
    let font_postscript_name = font.postscript_name();
    if font_postscript_name != descriptor_postscript_name {
        return Err("CTFont and URL-bound descriptor PostScript names differ".to_string());
    }
    Ok(())
}

pub(super) fn evidence(
    url_path: &str,
    descriptor: &CTFontDescriptor,
) -> Result<CoreTextDescriptorEvidence, String> {
    let axes = axes(descriptor)?;
    let coordinates = coordinates(descriptor)?;
    verify_weight_coordinate(&axes, &coordinates)?;
    Ok(CoreTextDescriptorEvidence {
        url_path: fs::canonicalize(url_path)
            .map_err(|error| format!("canonicalize CoreText evidence URL: {error}"))?
            .display()
            .to_string(),
        post_script_name: descriptor_postscript_name(descriptor)?,
        variation_axes: axes,
        variation_coordinates: coordinates,
    })
}

pub(super) fn descriptor_postscript_name(descriptor: &CTFontDescriptor) -> Result<String, String> {
    string_attribute(descriptor, postscript_name_key())
        .ok_or_else(|| "exact CoreText descriptor has no PostScript name".to_string())
}

fn string_attribute(descriptor: &CTFontDescriptor, key: CFString) -> Option<String> {
    let attributes = descriptor.attributes();
    attributes
        .find(key)?
        .downcast::<CFString>()
        .map(|value| value.to_string())
}

fn postscript_name_key() -> CFString {
    /* SAFETY: CoreTextの公開定数はプロセス存続中有効で、get-ruleで参照を保持する。 */
    unsafe { CFString::wrap_under_get_rule(kCTFontNameAttribute) }
}

fn variation_attribute_key() -> CFString {
    /* SAFETY: CoreTextの公開定数はプロセス存続中有効で、get-ruleで参照を保持する。 */
    unsafe { CFString::wrap_under_get_rule(kCTFontVariationAttribute) }
}

fn axes(descriptor: &CTFontDescriptor) -> Result<Vec<CoreTextVariationAxis>, String> {
    let axes = font::new_from_descriptor(descriptor, POSTSCRIPT_CHECK_SIZE)
        .get_variation_axes()
        .ok_or_else(|| "CoreText returned no variation-axis inventory".to_string())?;
    axis_values::parse_axes(&axes)
}

fn coordinates(descriptor: &CTFontDescriptor) -> Result<Vec<CoreTextVariationCoordinate>, String> {
    let variation_key = variation_attribute_key();
    let value = unsafe {
        CTFontDescriptorCopyAttribute(
            descriptor.as_concrete_TypeRef(),
            variation_key.as_concrete_TypeRef(),
        )
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    /* SAFETY: CoreTextのCopy APIが返した所有済み属性を型確認するまで保持する。 */
    let value = unsafe { CFType::wrap_under_create_rule(value) };
    let dictionary = value
        .downcast::<CFDictionary>()
        .ok_or_else(|| "CoreText variation coordinates are not a dictionary".to_string())?;
    let mut values = coordinate_pairs(&dictionary)?;
    values.sort_by_key(|coordinate| coordinate.identifier);
    axis_values::reject_duplicate_ids(values.iter().map(|coordinate| coordinate.identifier))?;
    Ok(values)
}

fn coordinate_pairs(
    coordinates: &CFDictionary,
) -> Result<Vec<CoreTextVariationCoordinate>, String> {
    let (keys, values) = coordinates.get_keys_and_values();
    keys.into_iter()
        .zip(values)
        .map(|(key, value)| parse_coordinate(key, value))
        .collect()
}

fn parse_coordinate(
    key: CFTypeRef,
    value: CFTypeRef,
) -> Result<CoreTextVariationCoordinate, String> {
    if key.is_null() || value.is_null() {
        return Err("CoreText returned a null variation-coordinate entry".to_string());
    }
    /* SAFETY: このCoreText属性はaxis IDから数値値へのCFDictionaryで、辞書が存続する間は要素も有効。 */
    let key = unsafe { CFType::wrap_under_get_rule(key) }
        .downcast::<CFNumber>()
        .ok_or_else(|| {
            "CoreText returned an invalid variation-coordinate identifier".to_string()
        })?;
    let key = axis_values::number_to_identifier(&key)?;
    /* SAFETY: 同じ属性契約により座標値はCFNumberで、辞書が存続する間は要素も有効。 */
    let value = unsafe { CFType::wrap_under_get_rule(value) }
        .downcast::<CFNumber>()
        .and_then(|number| number.to_f64())
        .filter(|value| value.is_finite())
        .ok_or_else(|| "CoreText returned an invalid variation coordinate".to_string())?;
    Ok(CoreTextVariationCoordinate {
        identifier: key,
        value,
    })
}

fn verify_weight_coordinate(
    axes: &[CoreTextVariationAxis],
    coordinates: &[CoreTextVariationCoordinate],
) -> Result<(), String> {
    let weight_axis = axes
        .iter()
        .find(|axis| axis.identifier == WEIGHT_AXIS)
        .ok_or_else(|| "exact CoreText descriptor exposes no variable weight axis".to_string())?;
    let weight = coordinates
        .iter()
        .find(|coordinate| coordinate.identifier == WEIGHT_AXIS)
        .ok_or_else(|| "exact CoreText instance exposes no weight coordinate".to_string())?;
    if weight.value < weight_axis.minimum_value || weight.value > weight_axis.maximum_value {
        return Err("CoreText weight coordinate is outside its reported axis bounds".to_string());
    }
    Ok(())
}
