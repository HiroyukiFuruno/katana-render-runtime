#[cfg(target_os = "macos")]
const EXPECTED_BOLD_WEIGHT: f64 = 700.0;
#[cfg(target_os = "macos")]
const CORETEXT_SINGLE_FONT_FIXTURE: &[u8] =
    include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
#[cfg(target_os = "macos")]
const TTC_HEADER_SIZE: usize = 20;

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
#[path = "identity_coretext_cases_tests.rs"]
mod tests;

#[cfg(target_os = "macos")]
pub(super) use tests::{
    coretext_fallback_rejects_collections_and_nonzero_face_indices,
    coretext_url_descriptor_proves_sfns_bold_variable_instance,
};
