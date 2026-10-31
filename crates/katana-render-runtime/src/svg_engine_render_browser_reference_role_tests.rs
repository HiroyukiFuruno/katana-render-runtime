use super::super::{
    Case, Comparison, Manifest, ONE_AXIS_CASE, validate_case_at, validate_manifest_at,
};
use super::valid_manifest;
use std::collections::BTreeSet;
use std::path::Path;

const UNKNOWN_FIELD: &str = "unrecognized";
const LAST_CASE_EXCLUSION: usize = 1;
const MISSING_TEST_ROOT: &str = "missing";

#[test]
fn manifest_and_case_shape_mutations_are_rejected() -> Result<(), String> {
    let manifest = valid_manifest()?;
    let mut wrong_count = manifest.clone();
    wrong_count.cases = manifest.cases[..manifest.cases.len() - LAST_CASE_EXCLUSION].to_vec();
    assert!(
        validate_manifest_at(
            &wrong_count,
            Path::new(MISSING_TEST_ROOT),
            Path::new(MISSING_TEST_ROOT)
        )
        .is_err()
    );

    let case = manifest.cases.first().ok_or("correction case missing")?;
    assert_case_rejected(case.clone(), BTreeSet::new(), |candidate| {
        candidate.input = "filters/feMorphology/not-listed.svg".to_owned();
    })?;
    assert_case_rejected(case.clone(), BTreeSet::from([case.input.clone()]), |_| {})?;
    assert_case_rejected(case.clone(), BTreeSet::new(), |candidate| {
        candidate.reference = "../upstream.png".to_owned();
    })?;
    assert_role_rejected(&manifest, ONE_AXIS_CASE, Comparison::BrowserPixels)?;
    assert_role_rejected(
        &manifest,
        "filters/feMorphology/empty-radius.svg",
        Comparison::OneAxisContract,
    )?;
    Ok(())
}

#[test]
fn manifest_deserializer_rejects_unknown_fields_and_comparison_roles() -> Result<(), String> {
    let bytes = std::fs::read(super::super::MANIFEST).map_err(|error| error.to_string())?;
    let original: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    for location in ["manifest", "browser", "render", "case"] {
        let mut value = original.clone();
        insert_unknown_field(&mut value, location)?;
        assert!(
            serde_json::from_value::<Manifest>(value).is_err(),
            "{location}"
        );
    }
    let mut unknown_role = original;
    set_unknown_comparison(&mut unknown_role)?;
    assert!(serde_json::from_value::<Manifest>(unknown_role).is_err());
    Ok(())
}

fn assert_case_rejected(
    mut case: Case,
    inputs: BTreeSet<String>,
    mutate: impl FnOnce(&mut Case),
) -> Result<(), String> {
    mutate(&mut case);
    let mut inputs = inputs;
    assert!(
        validate_case_at(
            &case,
            &mut inputs,
            Path::new(MISSING_TEST_ROOT),
            Path::new(MISSING_TEST_ROOT)
        )
        .is_err()
    );
    Ok(())
}

fn assert_role_rejected(
    manifest: &Manifest,
    input: &str,
    invalid_role: Comparison,
) -> Result<(), String> {
    let mut case = manifest
        .cases
        .iter()
        .find(|case| case.input == input)
        .ok_or_else(|| format!("manifest case missing: {input}"))?
        .clone();
    case.comparison = invalid_role;
    assert_case_rejected(case, BTreeSet::new(), |_| {})
}

fn insert_unknown_field(value: &mut serde_json::Value, location: &str) -> Result<(), String> {
    let target = match location {
        "manifest" => value,
        "browser" | "render" => value
            .get_mut(location)
            .ok_or_else(|| format!("{location} object missing"))?,
        "case" => value
            .get_mut("cases")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|cases| cases.first_mut())
            .ok_or("case array is empty")?,
        _ => return Err("unknown test location".to_owned()),
    };
    target
        .as_object_mut()
        .ok_or_else(|| format!("{location} is not an object"))?
        .insert(UNKNOWN_FIELD.to_owned(), serde_json::Value::Null);
    Ok(())
}

fn set_unknown_comparison(value: &mut serde_json::Value) -> Result<(), String> {
    let comparison = value
        .get_mut("cases")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|cases| cases.first_mut())
        .and_then(|case| case.get_mut("comparison"))
        .ok_or("case comparison missing")?;
    *comparison = serde_json::Value::String("unknown-role".to_owned());
    Ok(())
}
