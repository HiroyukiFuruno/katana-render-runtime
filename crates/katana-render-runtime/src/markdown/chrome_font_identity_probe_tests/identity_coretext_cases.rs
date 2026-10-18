use super::SelectedFont;
#[cfg(target_os = "macos")]
use super::identity::sha256;
#[cfg(target_os = "macos")]
use super::test_support::probe_single_capture;
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::Path;

#[cfg(target_os = "macos")]
const EXPECTED_BOLD_WEIGHT: f64 = 700.0;
#[cfg(target_os = "macos")]
const CORETEXT_SINGLE_FONT_FIXTURE: &[u8] =
    include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
#[cfg(target_os = "macos")]
const TTC_HEADER_SIZE: usize = 20;

#[cfg(target_os = "macos")]
pub(super) fn coretext_fallback_rejects_collections_and_nonzero_face_indices() {
    assert!(
        super::identity::verify_coretext_single_font_data_for_test(CORETEXT_SINGLE_FONT_FIXTURE, 0)
            .is_ok()
    );
    assert!(
        super::identity::verify_coretext_single_font_data_for_test(CORETEXT_SINGLE_FONT_FIXTURE, 1)
            .is_err()
    );
    let collection = minimal_collection_header();
    assert!(matches!(
        read_fonts::FileRef::new(&collection),
        Ok(read_fonts::FileRef::Collection(_))
    ));
    assert!(super::identity::verify_coretext_single_font_data_for_test(&collection, 0).is_err());
}

#[cfg(target_os = "macos")]
fn minimal_collection_header() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(TTC_HEADER_SIZE);
    bytes.extend_from_slice(b"ttcf");
    bytes.extend_from_slice(&[0, 1, 0, 0]);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&(TTC_HEADER_SIZE as u32).to_be_bytes());
    bytes
}

#[cfg(target_os = "macos")]
pub(super) fn coretext_url_descriptor_proves_sfns_bold_variable_instance() -> Result<(), String> {
    let selected = SelectedFont {
        family_name: ".SF NS".to_string(),
        post_script_name: ".SFNS-Bold".to_string(),
        glyph_count: 1,
        is_custom_font: false,
    };
    let capture = probe_single_capture("coretext-sfns-bold", selected.clone(), Vec::new())?;
    if capture.status != "resolved"
        || !capture.unknown_reasons.is_empty()
        || capture.font_provenance.len() != 1
    {
        return Err("CoreText .SFNS-Bold probe is not resolved".to_string());
    }
    let provenance = capture
        .font_provenance
        .first()
        .ok_or("CoreText font provenance is missing")?;
    let proof = provenance
        .core_text_descriptor
        .as_ref()
        .ok_or("CoreText descriptor proof is missing")?;
    verify_physical_binding(provenance, proof)?;
    verify_descriptor_identity(&selected, Path::new(&provenance.file_path), proof)?;
    verify_bold_weight_coordinate(proof)
}

#[cfg(target_os = "macos")]
fn verify_physical_binding(
    provenance: &super::FontProvenance,
    proof: &super::CoreTextDescriptorEvidence,
) -> Result<(), String> {
    if provenance.file_path != proof.url_path {
        return Err("CoreText physical file binding is incomplete".to_string());
    }
    let bytes = fs::read(&provenance.file_path).map_err(|error| error.to_string())?;
    if provenance.face_index != 0 || sha256(&bytes) != provenance.file_sha256 {
        return Err("CoreText physical file SHA or face index differs".to_string());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_descriptor_identity(
    selected: &SelectedFont,
    physical_path: &std::path::Path,
    proof: &super::CoreTextDescriptorEvidence,
) -> Result<(), String> {
    if proof.post_script_name != selected.post_script_name
        || proof.url_path != physical_path.display().to_string()
    {
        return Err("CoreText descriptor is not bound to the selected physical face".to_string());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_bold_weight_coordinate(proof: &super::CoreTextDescriptorEvidence) -> Result<(), String> {
    let weight_axis = u32::from_be_bytes(*b"wght");
    let coordinate = proof
        .variation_coordinates
        .iter()
        .find(|coordinate| coordinate.identifier == weight_axis)
        .ok_or("CoreText bold descriptor has no wght coordinate")?;
    if coordinate.value != EXPECTED_BOLD_WEIGHT {
        return Err(format!(
            "CoreText .SFNS-Bold wght coordinate was {}, expected measured {}",
            coordinate.value, EXPECTED_BOLD_WEIGHT
        ));
    }
    Ok(())
}
