use super::image_data;
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

const MAX_SUBRESOURCE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_BASE64_DATA_URL_BYTES: u64 = MAX_SUBRESOURCE_BYTES.div_ceil(3) * 4;

pub(super) fn load_text(url: &Url) -> Result<String, String> {
    let text = String::from_utf8(load_bytes(url)?)
        .map_err(|error| format!("subresource is not UTF-8: {error}"))?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

pub(super) fn image_data_url_is_decodable(reference: &str) -> bool {
    let Ok(url) = Url::parse(reference) else {
        return false;
    };
    if url.scheme() != "data" {
        return false;
    }
    load_bytes(&url)
        .and_then(|bytes| image_data::validate_data_url(&url, &bytes))
        .is_ok()
}

pub(super) fn load_image_data_url(url: &Url) -> Result<String, String> {
    if url.scheme() == "data" {
        image_data::validate_data_url(url, &load_bytes(url)?)?;
        return Ok(url.to_string());
    }
    let payload = load_image_bytes(url)?;
    let media_type =
        image_data::resolve_media_type(payload.declared_media_type.as_deref(), &payload.bytes)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload.bytes);
    Ok(format!("data:{media_type};base64,{encoded}"))
}

fn load_bytes(url: &Url) -> Result<Vec<u8>, String> {
    match url.scheme() {
        "data" => decode_data_url(url),
        "file" => std::fs::read(url.to_file_path().map_err(|_| "file URL is invalid")?)
            .map_err(|error| format!("local subresource could not be read: {error}")),
        "http" | "https" => load_http(url),
        scheme => Err(format!("unsupported subresource scheme: {scheme}")),
    }
}

struct ImagePayload {
    bytes: Vec<u8>,
    declared_media_type: Option<String>,
}

fn load_image_bytes(url: &Url) -> Result<ImagePayload, String> {
    if matches!(url.scheme(), "http" | "https") {
        let payload = load_http_payload(url)?;
        return Ok(ImagePayload {
            bytes: payload.bytes,
            declared_media_type: payload.content_type,
        });
    }
    Ok(ImagePayload {
        bytes: load_bytes(url)?,
        declared_media_type: None,
    })
}

fn load_http(url: &Url) -> Result<Vec<u8>, String> {
    Ok(load_http_payload(url)?.bytes)
}

struct HttpPayload {
    bytes: Vec<u8>,
    content_type: Option<String>,
}

fn load_http_payload(url: &Url) -> Result<HttpPayload, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .build()
        .into();
    let mut response = agent
        .get(url.as_str())
        .call()
        .map_err(|error| format!("network subresource could not be read: {error}"))?;
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|header| header.to_str().ok())
        .map(str::to_owned);
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_SUBRESOURCE_BYTES)
        .read_to_vec()
        .map_err(|error| format!("network subresource body could not be read: {error}"))?;
    Ok(HttpPayload {
        bytes,
        content_type,
    })
}

fn decode_data_url(url: &Url) -> Result<Vec<u8>, String> {
    let without_fragment = url
        .as_str()
        .split_once('#')
        .map_or(url.as_str(), |(source, _)| source);
    let source = &without_fragment["data:".len()..];
    let (metadata, payload) = source.split_once(',').ok_or("data URL has no payload")?;
    check_data_url_size(payload.len(), MAX_BASE64_DATA_URL_BYTES, "encoded")?;
    if metadata
        .split(';')
        .next_back()
        .is_some_and(|part| part.eq_ignore_ascii_case("base64"))
    {
        let decoded_payload = percent_decode_str(payload).collect::<Vec<u8>>();
        let bytes = decode_forgiving_base64(&decoded_payload)
            .map_err(|error| format!("data URL base64 payload is invalid: {error}"))?;
        check_data_url_size(bytes.len(), MAX_SUBRESOURCE_BYTES, "decoded")?;
        return Ok(bytes);
    }
    let bytes = percent_decode_str(payload).collect::<Vec<_>>();
    check_data_url_size(bytes.len(), MAX_SUBRESOURCE_BYTES, "decoded")?;
    Ok(bytes)
}

fn check_data_url_size(size: usize, limit: u64, representation: &str) -> Result<(), String> {
    if size as u64 > limit {
        return Err(format!(
            "data URL {representation} payload exceeds the {limit}-byte limit"
        ));
    }
    Ok(())
}

fn decode_forgiving_base64(payload: &[u8]) -> Result<Vec<u8>, base64::DecodeError> {
    let payload = payload
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
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

#[cfg(test)]
#[path = "transport_http_tests.rs"]
mod http_tests;
#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
