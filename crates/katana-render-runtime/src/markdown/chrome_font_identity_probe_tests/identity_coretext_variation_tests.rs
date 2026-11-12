use super::super::super::super::{CoreTextVariationAxis, CoreTextVariationCoordinate};
use super::{WEIGHT_AXIS, coordinate_pairs, parse_coordinate, verify_weight_coordinate};
use core_foundation::{
    base::{CFType, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};

const AXIS: CoreTextVariationAxis = CoreTextVariationAxis {
    identifier: WEIGHT_AXIS,
    minimum_value: 100.0,
    maximum_value: 900.0,
    default_value: 400.0,
};
const WEIGHT: CoreTextVariationCoordinate = CoreTextVariationCoordinate {
    identifier: WEIGHT_AXIS,
    value: 700.0,
};
type WeightErrorCase = (
    &'static [CoreTextVariationAxis],
    &'static [CoreTextVariationCoordinate],
    Option<&'static str>,
);

const WEIGHT_ERROR_CASES: [WeightErrorCase; 5] = [
    (
        &[],
        &[WEIGHT],
        Some("exact CoreText descriptor exposes no variable weight axis"),
    ),
    (
        &[AXIS],
        &[],
        Some("exact CoreText instance exposes no weight coordinate"),
    ),
    (
        &[AXIS],
        &[CoreTextVariationCoordinate {
            value: 99.0,
            ..WEIGHT
        }],
        Some("CoreText weight coordinate is outside its reported axis bounds"),
    ),
    (
        &[AXIS],
        &[CoreTextVariationCoordinate {
            value: 901.0,
            ..WEIGHT
        }],
        Some("CoreText weight coordinate is outside its reported axis bounds"),
    ),
    (&[AXIS], &[WEIGHT], None),
];
const NULL_ENTRY_ERROR: &str = "CoreText returned a null variation-coordinate entry";
const INVALID_IDENTIFIER_ERROR: &str =
    "CoreText returned an invalid variation-coordinate identifier";
const INVALID_COORDINATE_ERROR: &str = "CoreText returned an invalid variation coordinate";

#[test]
fn weight_coordinate_requires_an_axis_and_an_in_range_value() {
    for (axes, coordinates, expected_error) in WEIGHT_ERROR_CASES {
        assert_eq!(
            verify_weight_coordinate(axes, coordinates).err().as_deref(),
            expected_error
        );
    }
}

#[test]
fn static_font_descriptor_rejects_a_different_postscript_name() -> Result<(), String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    let descriptors = super::super::descriptors_from_url(&path)?;
    let descriptor = descriptors
        .iter()
        .next()
        .ok_or("CoreText returned no descriptor for the fixture font")?;
    let name = super::descriptor_postscript_name(&descriptor)?;
    super::verify_ctfont_descriptor_name(&descriptor, &name)?;
    assert_eq!(
        super::verify_ctfont_descriptor_name(&descriptor, "krr-unmatched-postscript-name")
            .err()
            .as_deref(),
        Some("CTFont and URL-bound descriptor PostScript names differ")
    );
    assert!(super::coordinates(&descriptor)?.is_empty());
    Ok(())
}

#[test]
fn coretext_evidence_rejects_a_missing_file_after_descriptor_lookup() -> Result<(), String> {
    let temp = super::super::super::super::test_support::TempDirectory::new()?;
    let path = temp.path().join("variable.ttf");
    let bytes = include_bytes!(
        "../../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/RobotoFlex.subset.ttf"
    );
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    let descriptors = super::super::descriptors_from_url(&path)?;
    let path_str = path.to_str().ok_or("fixture path is not UTF-8")?;
    let descriptor = descriptors
        .iter()
        .find(|descriptor| super::evidence(path_str, descriptor).is_ok())
        .ok_or("variable font has no verifiable descriptor")?;
    let missing = temp.path().join("missing.ttf");
    let missing_str = missing.to_str().ok_or("missing path is not UTF-8")?;
    let error = super::evidence(missing_str, &descriptor)
        .err()
        .ok_or("missing evidence file was accepted")?;
    assert!(error.starts_with("canonicalize CoreText evidence URL:"));
    Ok(())
}

#[test]
fn coordinate_pairs_parse_real_corefoundation_values_and_reject_bad_types() {
    let cases = [
        (CoordinateEntry::Valid, None),
        (
            CoordinateEntry::WrongKeyType,
            Some(INVALID_IDENTIFIER_ERROR),
        ),
        (
            CoordinateEntry::WrongValueType,
            Some(INVALID_COORDINATE_ERROR),
        ),
        (
            CoordinateEntry::NonFiniteValue,
            Some(INVALID_COORDINATE_ERROR),
        ),
    ];
    for (entry, expected_error) in cases {
        assert_coordinate_entry(entry, expected_error);
    }
}

#[test]
fn coordinate_parser_rejects_null_dictionary_entries_before_dereferencing() {
    let coordinate = CFNumber::from(WEIGHT.value);
    for null_key in [true, false] {
        let key = CFNumber::from(i64::from(WEIGHT_AXIS));
        let key_pointer = if null_key {
            std::ptr::null()
        } else {
            key.as_CFTypeRef()
        };
        let value_pointer = if null_key {
            coordinate.as_CFTypeRef()
        } else {
            std::ptr::null()
        };
        assert_eq!(
            parse_coordinate(key_pointer, value_pointer)
                .err()
                .as_deref(),
            Some(NULL_ENTRY_ERROR)
        );
    }
}

#[derive(Clone, Copy)]
enum CoordinateEntry {
    Valid,
    WrongKeyType,
    WrongValueType,
    NonFiniteValue,
}

fn assert_coordinate_entry(entry: CoordinateEntry, expected_error: Option<&str>) {
    let number_key = CFNumber::from(i64::from(WEIGHT_AXIS));
    let number_value = CFNumber::from(WEIGHT.value);
    let non_finite = CFNumber::from(f64::NAN);
    let text = CFString::new("not a CoreText number");
    let (key, value): (CFType, CFType) = match entry {
        CoordinateEntry::Valid => (number_key.as_CFType(), number_value.as_CFType()),
        CoordinateEntry::WrongKeyType => (text.as_CFType(), number_value.as_CFType()),
        CoordinateEntry::WrongValueType => (number_key.as_CFType(), text.as_CFType()),
        CoordinateEntry::NonFiniteValue => (number_key.as_CFType(), non_finite.as_CFType()),
    };
    let dictionary = CFDictionary::from_CFType_pairs(&[(key, value)]).into_untyped();
    let parsed = coordinate_pairs(&dictionary);
    assert_eq!(parsed.as_ref().err().map(String::as_str), expected_error);
    if expected_error.is_none() {
        assert_eq!(parsed.ok().as_deref(), Some(&[WEIGHT][..]));
    }
}
