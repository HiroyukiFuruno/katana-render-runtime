use super::{
    AXIS_DEFAULT_ATTRIBUTE, AXIS_IDENTIFIER_ATTRIBUTE, AXIS_MAXIMUM_ATTRIBUTE,
    AXIS_MINIMUM_ATTRIBUTE, number_to_identifier, parse_axes,
};
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};

type AxisDictionary = CFDictionary<CFString, CFType>;
type AxisArray = CFArray<AxisDictionary>;

const AXIS_IDENTIFIER: f64 = 0x7767_6874 as f64;
const WEIGHT_AXIS_MINIMUM: f64 = 100.0;
const WEIGHT_AXIS_MAXIMUM: f64 = 900.0;
const WEIGHT_AXIS_DEFAULT: f64 = 400.0;
const INVALID_ID_ERROR: &str = "CoreText returned an invalid variation identifier";
const OUT_OF_RANGE_ID_ERROR: &str = "CoreText returned an out-of-range variation identifier";
const INVALID_BOUNDS_ERROR: &str = "CoreText returned invalid variation-axis bounds";
const DUPLICATE_ID_ERROR: &str = "CoreText returned duplicate variation identifiers";

#[derive(Clone, Copy, Debug)]
enum AxisCase {
    MissingMaximum,
    WrongMaximumType,
    InvalidBounds,
    DuplicateIdentifier,
    FractionalIdentifier,
    NegativeIdentifier,
    OverflowIdentifier,
}

const MALFORMED_AXIS_CASES: [(AxisCase, Option<&str>); 7] = [
    (
        AxisCase::MissingMaximum,
        Some("CoreText variation axis is missing numeric NSCTVariationAxisMaximumValue"),
    ),
    (
        AxisCase::WrongMaximumType,
        Some("CoreText variation axis is missing numeric NSCTVariationAxisMaximumValue"),
    ),
    (AxisCase::InvalidBounds, Some(INVALID_BOUNDS_ERROR)),
    (AxisCase::DuplicateIdentifier, Some(DUPLICATE_ID_ERROR)),
    (AxisCase::FractionalIdentifier, Some(INVALID_ID_ERROR)),
    (AxisCase::NegativeIdentifier, Some(OUT_OF_RANGE_ID_ERROR)),
    (AxisCase::OverflowIdentifier, Some(OUT_OF_RANGE_ID_ERROR)),
];

#[test]
fn axis_identifier_preserves_u32_endpoints_in_both_cfnumber_encodings() {
    for identifier in [u32::MIN, u32::MAX] {
        for number in [
            CFNumber::from(i64::from(identifier)),
            CFNumber::from(f64::from(identifier)),
        ] {
            assert_eq!(number_to_identifier(&number), Ok(identifier));
        }
    }
}

#[test]
fn parse_axes_rejects_malformed_real_corefoundation_dictionaries() {
    for (case, expected_error) in MALFORMED_AXIS_CASES {
        let axes = case_axes(case);
        let result = parse_axes(&axes);
        assert_eq!(result.err().as_deref(), expected_error, "{case:?}");
    }
}

#[test]
fn parse_axes_sorts_valid_real_corefoundation_dictionaries() -> Result<(), String> {
    let dictionaries = [
        axis_dictionary(200.0, 0.0, Some(number(1.0)), 0.5),
        axis_dictionary(
            100.0,
            10.0,
            Some(number(WEIGHT_AXIS_MAXIMUM)),
            WEIGHT_AXIS_DEFAULT,
        ),
    ];
    let axes = CFArray::from_CFTypes(&dictionaries);
    let parsed = parse_axes(&axes)?;

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].identifier, 100);
    assert_eq!(parsed[0].minimum_value, 10.0);
    assert_eq!(parsed[0].maximum_value, WEIGHT_AXIS_MAXIMUM);
    assert_eq!(parsed[0].default_value, WEIGHT_AXIS_DEFAULT);
    assert_eq!(parsed[1].identifier, 200);
    Ok(())
}

fn case_axes(case: AxisCase) -> AxisArray {
    CFArray::from_CFTypes(&case_dictionaries(case))
}

fn case_dictionaries(case: AxisCase) -> Vec<AxisDictionary> {
    match case {
        AxisCase::DuplicateIdentifier => vec![valid_axis(), valid_axis()],
        AxisCase::MissingMaximum => vec![axis_dictionary(
            AXIS_IDENTIFIER,
            WEIGHT_AXIS_MINIMUM,
            None,
            WEIGHT_AXIS_DEFAULT,
        )],
        AxisCase::WrongMaximumType => vec![axis_dictionary(
            AXIS_IDENTIFIER,
            WEIGHT_AXIS_MINIMUM,
            Some(CFString::new("not a number").as_CFType()),
            WEIGHT_AXIS_DEFAULT,
        )],
        AxisCase::InvalidBounds => {
            vec![axis_dictionary(
                AXIS_IDENTIFIER,
                950.0,
                Some(number(WEIGHT_AXIS_MINIMUM)),
                WEIGHT_AXIS_DEFAULT,
            )]
        }
        AxisCase::FractionalIdentifier => vec![identifier_axis(0.5)],
        AxisCase::NegativeIdentifier => vec![identifier_axis(-1.0)],
        AxisCase::OverflowIdentifier => vec![identifier_axis(4_294_967_296.0)],
    }
}

fn identifier_axis(identifier: f64) -> AxisDictionary {
    axis_dictionary(
        identifier,
        WEIGHT_AXIS_MINIMUM,
        Some(number(WEIGHT_AXIS_MAXIMUM)),
        WEIGHT_AXIS_DEFAULT,
    )
}

fn valid_axis() -> AxisDictionary {
    axis_dictionary(
        AXIS_IDENTIFIER,
        WEIGHT_AXIS_MINIMUM,
        Some(number(WEIGHT_AXIS_MAXIMUM)),
        WEIGHT_AXIS_DEFAULT,
    )
}

fn axis_dictionary(
    identifier: f64,
    minimum: f64,
    maximum: Option<CFType>,
    default: f64,
) -> AxisDictionary {
    let mut attributes = vec![
        (CFString::new(AXIS_IDENTIFIER_ATTRIBUTE), number(identifier)),
        (CFString::new(AXIS_MINIMUM_ATTRIBUTE), number(minimum)),
        (CFString::new(AXIS_DEFAULT_ATTRIBUTE), number(default)),
    ];
    if let Some(maximum) = maximum {
        attributes.push((CFString::new(AXIS_MAXIMUM_ATTRIBUTE), maximum));
    }
    CFDictionary::from_CFType_pairs(&attributes)
}

fn number(value: f64) -> CFType {
    CFNumber::from(value).as_CFType()
}
