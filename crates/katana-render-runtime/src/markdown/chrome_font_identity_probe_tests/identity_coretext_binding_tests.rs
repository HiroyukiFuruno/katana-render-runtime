use super::super::super::test_support::{TempDirectory, physical_font_fixture};
use super::super::PhysicalFace;
use super::{SelectedFont, resolve_font};
use core_foundation::{base::CFType, dictionary::CFDictionary, string::CFString};
use core_text::font_descriptor::{new_from_attributes, new_from_postscript_name};
use resvg::usvg::fontdb::Database;
use std::{fs, path::Path};

const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
const VARIABLE_FONT_BYTES: &[u8] = include_bytes!(
    "../../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/RobotoFlex.subset.ttf"
);
const WEIGHT_AXIS: u32 = u32::from_be_bytes(*b"wght");
const BOLD_WEIGHT: f64 = 700.0;
const PATH_MISMATCH_ERROR: &str = "exact CoreText descriptor URL differs from its query file";
const IDENTITY_MISMATCH_ERROR: &str =
    "physical candidate changed before CoreText descriptor lookup";
const NAME_MISMATCH_ERROR: &str = "CoreText descriptor PostScript name differs from CDP";

#[test]
fn coretext_descriptor_path_must_match_the_real_query_font() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let query_path = temp.path().join("query.ttf");
    let other_path = temp.path().join("other.ttf");
    fs::write(&query_path, FONT_BYTES).map_err(|error| error.to_string())?;
    fs::write(&other_path, FONT_BYTES).map_err(|error| error.to_string())?;
    let (canonical, file_sha256, _) = super::super::physical_file(&query_path)?;
    let identity = PhysicalFace {
        path: canonical,
        face_index: 0,
        file_sha256,
    };

    assert_eq!(
        super::verify_descriptor_path(&query_path, &identity.path, &identity),
        Ok(())
    );
    assert_eq!(
        super::verify_descriptor_path(&other_path, &identity.path, &identity)
            .err()
            .as_deref(),
        Some(PATH_MISMATCH_ERROR)
    );
    Ok(())
}

#[test]
fn coretext_default_descriptor_url_must_match_query_file() -> Result<(), String> {
    let (_temp, database, selected) = physical_font_fixture()?;
    let face = database
        .faces()
        .next()
        .ok_or("fontdb did not load the real font")?;
    let identity = super::super::face_identity(&database, face)?;
    let attributes: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[]);
    let descriptor = new_from_attributes(&attributes);
    let descriptor_path = descriptor
        .font_path()
        .ok_or("CoreText did not provide its default descriptor URL")?;
    let canonical_descriptor_path =
        fs::canonicalize(&descriptor_path).map_err(|error| error.to_string())?;

    assert_ne!(canonical_descriptor_path.as_path(), identity.path.as_path());

    assert_eq!(
        super::verify_descriptor_binding(
            &descriptor,
            &identity.path,
            &identity,
            &selected.post_script_name,
        )
        .err()
        .as_deref(),
        Some(PATH_MISMATCH_ERROR)
    );
    Ok(())
}

#[test]
fn coretext_rejects_changed_physical_identity_before_lookup() -> Result<(), String> {
    let (_temp, database, selected) = physical_font_fixture()?;
    let face = database
        .faces()
        .next()
        .ok_or("fontdb did not load the real font")?;
    let mut identity = super::super::face_identity(&database, face)?;
    identity.file_sha256 = "0".repeat(64);
    let identity_path = identity.path.clone();
    let error = super::resolve_file(&identity_path, identity, &selected.post_script_name)
        .err()
        .ok_or("changed physical identity must be rejected")?;
    assert_eq!(error, IDENTITY_MISMATCH_ERROR);
    Ok(())
}

#[test]
fn coretext_rejects_a_candidate_removed_before_lookup() -> Result<(), String> {
    let (_temp, database, selected) = physical_font_fixture()?;
    let face = database.faces().next().ok_or("font face missing")?;
    let identity = super::super::face_identity(&database, face)?;
    let path = identity.path.clone();
    fs::remove_file(&path).map_err(|error| error.to_string())?;
    let error = super::resolve_file(&path, identity, &selected.post_script_name)
        .err()
        .ok_or("removed candidate must be rejected")?;
    assert!(error.starts_with("canonicalize font:"), "{error}");
    Ok(())
}

#[test]
fn coretext_rejects_descriptor_postscript_name_mismatch() -> Result<(), String> {
    let (temp, _database, _selected) = physical_font_fixture()?;
    let path = temp.path().join("physical-font.ttf");
    let descriptors = super::descriptors_from_url(&path)?;
    let descriptor = descriptors
        .iter()
        .next()
        .ok_or("CoreText returned no descriptor for the fixture font")?;
    let error = super::verify_descriptor_name(&descriptor, "krr-unmatched-postscript-name")
        .err()
        .ok_or("descriptor name mismatch must be rejected")?;
    assert_eq!(error, NAME_MISMATCH_ERROR);
    Ok(())
}

#[test]
fn coretext_lookup_rejects_a_missing_font_file() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let path = temp.path().join("missing.ttf");
    let result = super::descriptors_from_url(&path);
    match result {
        Err(error) => assert_eq!(error, "CoreText URL descriptor lookup returned null"),
        Ok(descriptors) => assert!(descriptors.is_empty()),
    }
    Ok(())
}

#[test]
fn coretext_rejects_a_non_font_candidate_with_matching_physical_identity() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let path = temp.path().join("not-a-font.ttf");
    fs::write(&path, b"not a font").map_err(|error| error.to_string())?;
    let (canonical, file_sha256, _) = super::super::physical_file(&path)?;
    let identity = PhysicalFace {
        path: canonical.clone(),
        face_index: 0,
        file_sha256,
    };
    match super::resolve_file(&canonical, identity, "unmatched-font") {
        Err(error) => assert_eq!(error, "CoreText URL descriptor lookup returned null"),
        Ok(matches) => assert!(matches.is_empty()),
    }
    Ok(())
}

#[test]
fn coretext_lookup_skips_descriptors_with_a_different_name() -> Result<(), String> {
    let (_temp, database, _) = physical_font_fixture()?;
    let face = database.faces().next().ok_or("font face missing")?;
    let identity = super::super::face_identity(&database, face)?;
    let path = identity.path.clone();
    let matches = super::resolve_file(&path, identity, "krr-unmatched-postscript-name")?;
    assert!(matches.is_empty());
    Ok(())
}

#[test]
fn coretext_binding_rejects_a_named_descriptor_without_a_physical_url() -> Result<(), String> {
    let (_temp, database, _) = physical_font_fixture()?;
    let face = database.faces().next().ok_or("font face missing")?;
    let identity = super::super::face_identity(&database, face)?;
    let name = "Krr-Unregistered-Descriptor-Without-A-Url";
    let descriptor = new_from_postscript_name(&CFString::new(name));
    assert_eq!(
        super::verify_descriptor_binding(&descriptor, &identity.path, &identity, name)
            .err()
            .as_deref(),
        Some("exact CoreText descriptor has no physical URL")
    );
    Ok(())
}

#[test]
fn coretext_binding_requires_matching_name_and_existing_file() -> Result<(), String> {
    let (_temp, database, selected) = physical_font_fixture()?;
    let face = database.faces().next().ok_or("font face missing")?;
    let identity = super::super::face_identity(&database, face)?;
    let descriptors = super::descriptors_from_url(&identity.path)?;
    let descriptor = descriptors
        .iter()
        .next()
        .ok_or("fixture descriptor missing")?;
    let verify =
        |name| super::verify_descriptor_binding(&descriptor, &identity.path, &identity, name);
    let expected_name = selected.post_script_name.as_str();
    assert_eq!(Path::new(&verify(expected_name)?), identity.path);
    assert_eq!(
        verify("krr-unmatched-postscript-name").err().as_deref(),
        Some(NAME_MISMATCH_ERROR)
    );
    fs::remove_file(&identity.path).map_err(|error| error.to_string())?;
    let error = verify(expected_name)
        .err()
        .ok_or("removed descriptor URL accepted")?;
    assert!(
        error.starts_with("canonicalize CoreText descriptor URL:"),
        "{error}"
    );
    Ok(())
}

#[test]
fn coretext_rejects_the_same_named_variable_face_in_distinct_files() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let first = temp.path().join("first-variable.ttf");
    let second = temp.path().join("second-variable.ttf");
    for path in [&first, &second] {
        fs::write(path, VARIABLE_FONT_BYTES).map_err(|error| error.to_string())?;
    }
    let mut database = Database::new();
    database
        .load_font_file(&first)
        .map_err(|error| error.to_string())?;
    let selected = selected_named_font(&database, &first)?;
    let control = resolve_font(&database, &selected)?;
    assert_eq!(
        control.identity.path,
        fs::canonicalize(&first).map_err(|error| error.to_string())?
    );

    database
        .load_font_file(&second)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        resolve_font(&database, &selected).err().as_deref(),
        Some("multiple URL-bound CoreText physical face matches are ambiguous")
    );
    Ok(())
}

fn selected_named_font(database: &Database, path: &Path) -> Result<SelectedFont, String> {
    let family_name = database
        .faces()
        .next()
        .and_then(|face| face.families.first())
        .map(|(name, _)| name.clone())
        .ok_or("variable font family missing")?;
    Ok(SelectedFont {
        family_name,
        post_script_name: named_bold_face(path)?,
        glyph_count: 1,
        is_custom_font: false,
    })
}

fn named_bold_face(path: &Path) -> Result<String, String> {
    let url_path = path.to_str().ok_or("fixture path must be UTF-8")?;
    let descriptors = super::descriptors_from_url(path)?;
    descriptors
        .iter()
        .find_map(|descriptor| {
            let evidence = super::variations::evidence(url_path, &descriptor).ok()?;
            evidence
                .variation_coordinates
                .iter()
                .any(|coordinate| {
                    coordinate.identifier == WEIGHT_AXIS && coordinate.value == BOLD_WEIGHT
                })
                .then_some(evidence.post_script_name)
        })
        .ok_or_else(|| "CoreText bold instance missing from bundled font".to_owned())
}
