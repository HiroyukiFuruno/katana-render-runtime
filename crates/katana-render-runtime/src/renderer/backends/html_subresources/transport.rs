use super::image_data;
use base64::Engine as _;
use url::Url;

#[path = "transport_data_url.rs"]
mod data_url;
#[cfg(test)]
use data_url::{
    MAX_BASE64_DATA_URL_BYTES, MAX_PERCENT_ENCODED_BASE64_DATA_URL_BYTES, check_data_url_size,
    decode_forgiving_base64, normalize_forgiving_base64,
};
use data_url::{MAX_SUBRESOURCE_BYTES, decode_data_url};

pub(super) fn load_text(url: &Url) -> Result<String, String> {
    let bytes = load_bytes(url)?;
    let text = String::from_utf8(bytes).map_err(|error| {
        #[cfg(test)]
        super::resource_audit::record_failure(url.as_str(), &error.to_string());
        format!("subresource is not UTF-8: {error}")
    })?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

pub(super) fn image_data_url_is_decodable(reference: &str) -> bool {
    let Ok(url) = Url::parse(reference) else {
        return false;
    };
    if url.scheme() != "data" {
        return false;
    }
    let result = load_bytes(&url).and_then(|bytes| image_data::validate_data_url(&url, &bytes));
    #[cfg(test)]
    if let Err(error) = &result {
        super::resource_audit::record_failure(url.as_str(), error);
    }
    result.is_ok()
}

pub(super) fn load_image_data_url(url: &Url) -> Result<String, String> {
    if url.scheme() == "data" {
        let bytes = load_bytes(url)?;
        let validation = image_data::validate_data_url(url, &bytes);
        #[cfg(test)]
        if let Err(error) = &validation {
            super::resource_audit::record_failure(url.as_str(), error);
        }
        validation?;
        return Ok(url.to_string());
    }
    let payload = load_image_bytes(url)?;
    let media_type_result =
        image_data::resolve_media_type(payload.declared_media_type.as_deref(), &payload.bytes);
    #[cfg(test)]
    if let Err(error) = &media_type_result {
        super::resource_audit::record_failure(url.as_str(), error);
    }
    let media_type = media_type_result?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload.bytes);
    Ok(format!("data:{media_type};base64,{encoded}"))
}

fn load_bytes(url: &Url) -> Result<Vec<u8>, String> {
    let result = match url.scheme() {
        "data" => decode_data_url(url),
        "file" => match url.to_file_path() {
            Ok(path) => std::fs::read(path)
                .map_err(|error| format!("local subresource could not be read: {error}")),
            Err(()) => Err("file URL is invalid".to_owned()),
        },
        "http" | "https" => load_http(url),
        scheme => Err(format!("unsupported subresource scheme: {scheme}")),
    };
    #[cfg(test)]
    if !matches!(url.scheme(), "http" | "https") {
        match &result {
            Ok(bytes) => {
                super::resource_audit::record_success(url.as_str(), bytes);
            }
            Err(error) => {
                super::resource_audit::record_failure(url.as_str(), error);
            }
        }
    }
    result
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
    let result = load_http_payload_inner(url);
    #[cfg(test)]
    match &result {
        Ok(payload) => super::resource_audit::record_success(url.as_str(), &payload.bytes),
        Err(error) => super::resource_audit::record_failure(url.as_str(), error),
    }
    result
}

fn load_http_payload_inner(url: &Url) -> Result<HttpPayload, String> {
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

#[cfg(test)]
#[path = "transport_http_tests.rs"]
mod http_tests;
#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
