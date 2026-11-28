use base64::{Engine as _, engine::general_purpose::GeneralPurpose};
use percent_encoding::percent_decode_str;
use url::Url;

const BASE64_BLOCK_SIZE: usize = 4;
const BASE64_REMAINDER_TWO: usize = 2;
const BASE64_REMAINDER_THREE: usize = 3;

const FORGIVING_BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    base64::engine::general_purpose::GeneralPurposeConfig::new()
        .with_decode_allow_trailing_bits(true),
);

pub(super) const MAX_SUBRESOURCE_BYTES: u64 = 8 * 1024 * 1024;
pub(super) const MAX_BASE64_DATA_URL_BYTES: u64 = MAX_SUBRESOURCE_BYTES.div_ceil(3) * 4;
pub(super) const MAX_PERCENT_ENCODED_DATA_URL_BYTES: u64 = MAX_SUBRESOURCE_BYTES * 3;
pub(super) const MAX_PERCENT_ENCODED_BASE64_DATA_URL_BYTES: u64 = MAX_BASE64_DATA_URL_BYTES * 3;

pub(super) fn decode_data_url(url: &Url) -> Result<Vec<u8>, String> {
    let without_fragment = url
        .as_str()
        .split_once('#')
        .map_or(url.as_str(), |(source, _)| source);
    let source = &without_fragment["data:".len()..];
    let (metadata, payload) = source.split_once(',').ok_or("data URL has no payload")?;
    if uses_base64_data_url_encoding(metadata) {
        return decode_base64_data_url_payload(payload);
    }
    decode_percent_data_url_payload(payload)
}

fn uses_base64_data_url_encoding(metadata: &str) -> bool {
    metadata
        .split(';')
        .next_back()
        .is_some_and(|part| part.eq_ignore_ascii_case("base64"))
}

fn decode_base64_data_url_payload(payload: &str) -> Result<Vec<u8>, String> {
    check_data_url_size(
        payload.len(),
        MAX_PERCENT_ENCODED_BASE64_DATA_URL_BYTES,
        "encoded",
    )?;
    let decoded_payload = percent_decode_str(payload).collect::<Vec<u8>>();
    let normalized_payload = normalize_forgiving_base64(&decoded_payload);
    check_data_url_size(
        normalized_payload.len(),
        MAX_BASE64_DATA_URL_BYTES,
        "base64 encoded",
    )?;
    let bytes = decode_forgiving_base64(&normalized_payload)
        .map_err(|error| format!("data URL base64 payload is invalid: {error}"))?;
    check_data_url_size(bytes.len(), MAX_SUBRESOURCE_BYTES, "decoded")?;
    Ok(bytes)
}

fn decode_percent_data_url_payload(payload: &str) -> Result<Vec<u8>, String> {
    check_data_url_size(payload.len(), MAX_PERCENT_ENCODED_DATA_URL_BYTES, "encoded")?;
    let bytes = percent_decode_str(payload).collect::<Vec<_>>();
    check_data_url_size(bytes.len(), MAX_SUBRESOURCE_BYTES, "decoded")?;
    Ok(bytes)
}

pub(super) fn check_data_url_size(
    size: usize,
    limit: u64,
    representation: &str,
) -> Result<(), String> {
    if size as u64 > limit {
        return Err(format!(
            "data URL {representation} payload exceeds the {limit}-byte limit"
        ));
    }
    Ok(())
}

pub(super) fn decode_forgiving_base64(payload: &[u8]) -> Result<Vec<u8>, base64::DecodeError> {
    let payload = normalize_forgiving_base64(payload);
    if payload.contains(&b'=') {
        return FORGIVING_BASE64.decode(payload);
    }
    let padding = match payload.len() % BASE64_BLOCK_SIZE {
        0 => 0,
        BASE64_REMAINDER_TWO => BASE64_REMAINDER_TWO,
        BASE64_REMAINDER_THREE => 1,
        _ => return FORGIVING_BASE64.decode(payload),
    };
    let mut padded = payload;
    padded.extend(std::iter::repeat_n(b'=', padding));
    FORGIVING_BASE64.decode(padded)
}

pub(super) fn normalize_forgiving_base64(payload: &[u8]) -> Vec<u8> {
    payload
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect()
}
