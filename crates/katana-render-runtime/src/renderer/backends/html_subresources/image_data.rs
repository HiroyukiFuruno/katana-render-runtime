use image::ImageDecoder as _;
use std::io::Cursor;
use url::Url;

const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_IMAGE_DECODED_BYTES: u64 = 64 * 1024 * 1024;
const SVG_NAMESPACE_URI: &str = "http://www.w3.org/2000/svg";

pub(super) fn resolve_media_type(
    declared_media_type: Option<&str>,
    bytes: &[u8],
) -> Result<&'static str, String> {
    if let Some(declared_media_type) = declared_media_type {
        let media_type = supported_media_type(declared_media_type)
            .ok_or_else(|| format!("unsupported image media type: {declared_media_type}"))?;
        if validate_bytes(media_type, bytes).is_ok() {
            return Ok(media_type);
        }
    }

    sniff_media_type(bytes).ok_or_else(|| "subresource is not a supported image".to_string())
}

pub(super) fn validate_data_url(url: &Url, bytes: &[u8]) -> Result<(), String> {
    let source = &url.as_str()["data:".len()..];
    let (metadata, _) = source.split_once(',').ok_or("data URL has no payload")?;
    let media_type = metadata
        .split_once(';')
        .map_or(metadata, |(media_type, _)| media_type)
        .trim();
    if !media_type.to_ascii_lowercase().starts_with("image/") {
        return Err(format!("data URL media type is not an image: {media_type}"));
    }
    validate_bytes(media_type, bytes)
}

pub(super) fn validate_bytes(media_type: &str, bytes: &[u8]) -> Result<(), String> {
    match media_type.to_ascii_lowercase().as_str() {
        "image/png" => valid_raster(bytes, image::ImageFormat::Png, media_type),
        "image/gif" => valid_raster(bytes, image::ImageFormat::Gif, media_type),
        "image/jpeg" => valid_raster(bytes, image::ImageFormat::Jpeg, media_type),
        "image/webp" => valid_raster(bytes, image::ImageFormat::WebP, media_type),
        "image/svg+xml" => valid_svg(bytes),
        unsupported => Err(format!("unsupported image media type: {unsupported}")),
    }
}

fn valid_raster(bytes: &[u8], format: image::ImageFormat, media_type: &str) -> Result<(), String> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_DECODED_BYTES);

    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|error| format!("{media_type} payload could not be decoded: {error}"))?;
    let (width, height) = decoder.dimensions();
    let decoded_bytes =
        u64::from(width) * u64::from(height) * u64::from(decoder.color_type().bytes_per_pixel());
    if decoded_bytes > MAX_IMAGE_DECODED_BYTES {
        return Err(format!(
            "{media_type} decoded payload exceeds the {MAX_IMAGE_DECODED_BYTES}-byte limit"
        ));
    }

    image::DynamicImage::from_decoder(decoder)
        .map(|_| ())
        .map_err(|error| format!("{media_type} payload could not be decoded: {error}"))
}

fn supported_media_type(content_type: &str) -> Option<&'static str> {
    match content_type
        .split(';')
        .next()?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/gif" => Some("image/gif"),
        "image/jpeg" => Some("image/jpeg"),
        "image/png" => Some("image/png"),
        "image/svg+xml" => Some("image/svg+xml"),
        "image/webp" => Some("image/webp"),
        _ => None,
    }
}

fn sniff_media_type(bytes: &[u8]) -> Option<&'static str> {
    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    const GIF87A_HEADER: &[u8] = b"GIF87a";
    const GIF89A_HEADER: &[u8] = b"GIF89a";
    const JPEG_START_OF_IMAGE: &[u8] = &[0xff, 0xd8];
    const RIFF_SIGNATURE: &[u8] = b"RIFF";
    const WEBP_SIGNATURE: &[u8] = b"WEBP";
    if bytes.starts_with(PNG_SIGNATURE) {
        return Some("image/png");
    }
    if bytes.starts_with(GIF87A_HEADER) || bytes.starts_with(GIF89A_HEADER) {
        return Some("image/gif");
    }
    if bytes.starts_with(JPEG_START_OF_IMAGE) {
        return Some("image/jpeg");
    }
    if is_webp(bytes, RIFF_SIGNATURE, WEBP_SIGNATURE) {
        return Some("image/webp");
    }
    valid_svg(bytes).is_ok().then_some("image/svg+xml")
}

fn is_webp(bytes: &[u8], riff_signature: &[u8], webp_signature: &[u8]) -> bool {
    const WEBP_SIGNATURE_OFFSET: usize = 8;
    const WEBP_SIGNATURE_END: usize = WEBP_SIGNATURE_OFFSET + 4;
    bytes.starts_with(riff_signature)
        && bytes.get(WEBP_SIGNATURE_OFFSET..WEBP_SIGNATURE_END) == Some(webp_signature)
}

fn valid_svg(bytes: &[u8]) -> Result<(), String> {
    let document = xmltree::Element::parse(bytes)
        .map_err(|error| format!("image/svg+xml payload could not be decoded: {error}"))?;
    if document.name == "svg" && document.namespace.as_deref() == Some(SVG_NAMESPACE_URI) {
        Ok(())
    } else {
        Err("image/svg+xml payload root is not an SVG element".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_IMAGE_DIMENSION, resolve_media_type, supported_media_type, validate_bytes,
        validate_data_url,
    };
    use url::Url;

    #[test]
    fn enormous_declared_png_dimensions_are_rejected_before_materialization() {
        let png = png_header_with_dimensions(MAX_IMAGE_DIMENSION + 1, 1);
        assert!(validate_bytes("image/png", &png).is_err());
    }

    #[test]
    fn declared_content_types_are_limited_to_supported_images() {
        assert_eq!(supported_media_type("image/gif"), Some("image/gif"));
        assert_eq!(
            supported_media_type("image/jpeg; charset=binary"),
            Some("image/jpeg")
        );
        assert_eq!(
            supported_media_type(" image/png ; charset=binary"),
            Some("image/png")
        );
        assert_eq!(supported_media_type("image/svg+xml"), Some("image/svg+xml"));
        assert_eq!(supported_media_type("image/webp"), Some("image/webp"));
        assert!(resolve_media_type(Some("text/plain"), b"").is_err());
    }

    #[test]
    fn unsupported_payloads_and_data_url_shapes_are_rejected() -> Result<(), url::ParseError> {
        assert!(resolve_media_type(None, b"not an image").is_err());
        assert_eq!(
            validate_data_url(&Url::parse("data:image/png")?, b""),
            Err("data URL has no payload".to_string())
        );
        assert_eq!(
            validate_bytes("image/bmp", b""),
            Err("unsupported image media type: image/bmp".to_string())
        );
        Ok(())
    }

    #[test]
    fn image_signatures_are_sniffed_without_decoding_the_payload() {
        assert_eq!(
            resolve_media_type(None, b"\x89PNG\r\n\x1a\n"),
            Ok("image/png")
        );
        assert_eq!(resolve_media_type(None, &[0xff, 0xd8]), Ok("image/jpeg"));
        assert_eq!(
            resolve_media_type(None, b"RIFF\0\0\0\0WEBP"),
            Ok("image/webp")
        );
    }

    #[test]
    fn invalid_declared_type_falls_back_to_the_sniffed_image_type() {
        const GIF: &[u8] =
            b"GIF89a\x01\0\x01\0\x80\0\0\0\0\0\xff\xff\xff,\0\0\0\0\x01\0\x01\0\0\x02\x01L\0;";

        assert_eq!(resolve_media_type(Some("image/png"), GIF), Ok("image/gif"));
    }

    #[test]
    fn unsupported_declared_type_is_rejected_even_when_bytes_sniff_as_an_image() {
        const GIF: &[u8] =
            b"GIF89a\x01\0\x01\0\x80\0\0\0\0\0\xff\xff\xff,\0\0\0\0\x01\0\x01\0\0\x02\x01L\0;";

        assert!(resolve_media_type(Some("text/plain"), GIF).is_err());
    }

    #[test]
    fn svg_root_name_is_case_sensitive() {
        assert!(
            validate_bytes(
                "image/svg+xml",
                br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#
            )
            .is_ok()
        );
        assert!(
            validate_bytes(
                "image/svg+xml",
                br#"<SVG xmlns="http://www.w3.org/2000/svg"/>"#
            )
            .is_err()
        );
    }

    #[test]
    fn svg_root_must_use_the_svg_namespace() {
        assert!(validate_bytes("image/svg+xml", br#"<x:svg xmlns:x="urn:not-svg"/>"#).is_err());
    }

    fn png_header_with_dimensions(width: u32, height: u32) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = b"IHDR".to_vec();
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[8, 6, 0, 0, 0]);
        png.extend_from_slice(&(13_u32).to_be_bytes());
        png.extend_from_slice(&header);
        png.extend_from_slice(&png_crc32(&header).to_be_bytes());
        png
    }

    fn png_crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }
}
