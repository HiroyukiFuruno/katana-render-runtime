use super::*;
use crate::renderer::backends::html_artifact::types::MAX_ARTIFACT_BYTES;
use crate::renderer::backends::html_browser::{
    HtmlBrowserOrigin, HtmlBrowserPixelFormat, HtmlBrowserViewport,
};

const FINGERPRINT_LENGTH: usize = 64;
const FRAME_BYTES: usize = 4;
const TEST_SCROLL_Y: f32 = 2.0;
const TEST_CONTENT_HEIGHT: f32 = 4.0;

fn artifact() -> Result<HtmlFrameArtifact, Box<dyn std::error::Error>> {
    let origin = HtmlBrowserOrigin::parse("https://example.test/library")?;
    let viewport = HtmlBrowserViewport::new(1, 1, 1.0)?;
    let frame = HtmlBrowserFrame::new(
        1,
        origin,
        viewport,
        HtmlBrowserPixelFormat::Rgba8,
        vec![0; FRAME_BYTES],
    )?
    .with_layout_metrics(TEST_SCROLL_Y, TEST_CONTENT_HEIGHT);
    let provenance = HtmlArtifactProvenance {
        source_sha256: "a".repeat(FINGERPRINT_LENGTH),
        origin: frame.origin.clone(),
        runtime_version: env!("CARGO_PKG_VERSION").into(),
        viewport,
        render_config_fingerprint: "b".repeat(FINGERPRINT_LENGTH),
        resource_content_fingerprint: "c".repeat(FINGERPRINT_LENGTH),
        font_content_fingerprint: "d".repeat(FINGERPRINT_LENGTH),
    };
    Ok(HtmlFrameArtifact::new(provenance, frame)?)
}

#[test]
fn validation_rejects_format_digest_provenance_and_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    assert_format_rejected()?;
    assert_digest_and_origin_rejected()?;
    assert_metadata_rejected()?;
    Ok(())
}

fn assert_format_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut invalid = artifact()?;
    invalid.format_version = 0;
    assert!(matches!(
        invalid.validate(),
        Err(HtmlArtifactError::UnsupportedFormatVersion(0))
    ));

    Ok(())
}

fn assert_digest_and_origin_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut invalid = artifact()?;
    invalid.frame_sha256 = "0".repeat(FINGERPRINT_LENGTH);
    assert!(matches!(
        invalid.validate(),
        Err(HtmlArtifactError::Corrupt)
    ));

    let mut invalid = artifact()?;
    invalid.frame.origin = HtmlBrowserOrigin::parse("https://other.test/")?;
    invalid.frame_sha256 = frame_digest(&invalid.frame);
    assert!(matches!(
        invalid.validate(),
        Err(HtmlArtifactError::ProvenanceMismatch)
    ));

    Ok(())
}

fn assert_metadata_rejected() -> Result<(), Box<dyn std::error::Error>> {
    for mutation in [
        |frame: &mut HtmlBrowserFrame| frame.generation = 0,
        |frame: &mut HtmlBrowserFrame| frame.scroll_y = -1.0,
        |frame: &mut HtmlBrowserFrame| frame.content_height = -1.0,
        |frame: &mut HtmlBrowserFrame| frame.content_height = f32::NAN,
    ] {
        let mut invalid = artifact()?;
        mutation(&mut invalid.frame);
        invalid.frame_sha256 = frame_digest(&invalid.frame);
        assert!(matches!(
            invalid.validate(),
            Err(HtmlArtifactError::InvalidProvenance(_))
        ));
    }
    Ok(())
}

#[test]
fn validation_rejects_buffer_and_encoding_limits() -> Result<(), Box<dyn std::error::Error>> {
    assert_buffer_rejected()?;
    assert_overflow_rejected()?;
    let valid = artifact()?;
    assert!(matches!(
        valid.encode_with_limit(1),
        Err(HtmlArtifactError::ArtifactTooLarge)
    ));
    assert!(matches!(
        HtmlFrameArtifact::decode_with_limit(b"{}", MAX_ARTIFACT_BYTES),
        Err(HtmlArtifactError::Corrupt)
    ));
    assert!(matches!(
        HtmlFrameArtifact::decode_with_limit(b"123", 2),
        Err(HtmlArtifactError::ArtifactTooLarge)
    ));
    Ok(())
}

#[test]
fn decoding_requires_exact_contract_fields_and_limit() -> Result<(), Box<dyn std::error::Error>> {
    let valid = artifact()?.encode_with_limit(MAX_ARTIFACT_BYTES)?;
    assert!(HtmlFrameArtifact::decode_with_limit(&valid, valid.len() as u64).is_ok());

    let mut value: serde_json::Value = serde_json::from_slice(&valid)?;
    value["unexpected"] = serde_json::json!(true);
    let unknown = serde_json::to_vec(&value)?;
    assert!(matches!(
        HtmlFrameArtifact::decode_with_limit(&unknown, MAX_ARTIFACT_BYTES),
        Err(HtmlArtifactError::Corrupt)
    ));
    Ok(())
}

fn assert_buffer_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut invalid = artifact()?;
    invalid.frame.pixels.clear();
    invalid.frame_sha256 = frame_digest(&invalid.frame);
    assert!(matches!(
        invalid.validate(),
        Err(HtmlArtifactError::InvalidProvenance(_))
    ));

    Ok(())
}

fn assert_overflow_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut invalid = artifact()?;
    let overflow = HtmlBrowserViewport {
        width: u32::MAX,
        height: u32::MAX,
        device_scale_factor: 1.0,
    };
    invalid.frame.viewport = overflow;
    invalid.provenance.viewport = overflow;
    invalid.frame.pixels.clear();
    invalid.frame_sha256 = frame_digest(&invalid.frame);
    assert!(matches!(
        invalid.validate(),
        Err(HtmlArtifactError::InvalidProvenance(message))
            if message == "frame dimensions overflow"
    ));

    Ok(())
}

#[test]
fn canonical_digest_includes_frame_metadata_and_pixels() -> Result<(), Box<dyn std::error::Error>> {
    let base = artifact()?;
    let base_digest = frame_digest(&base.frame);
    let mut mutations = Vec::new();
    let mut generation = base.frame.clone();
    generation.generation += 1;
    mutations.push(generation);
    let mut viewport = base.frame.clone();
    viewport.viewport.device_scale_factor = 2.0;
    mutations.push(viewport);
    let mut scroll = base.frame.clone();
    scroll.scroll_y += 1.0;
    mutations.push(scroll);
    let mut content_height = base.frame.clone();
    content_height.content_height += 1.0;
    mutations.push(content_height);
    let mut origin = base.frame.clone();
    origin.origin = HtmlBrowserOrigin::parse("https://example.test/other")?;
    mutations.push(origin);
    let mut pixels = base.frame.clone();
    pixels.pixels[0] = 1;
    mutations.push(pixels);

    for frame in mutations {
        assert_ne!(frame_digest(&frame), base_digest);
    }
    assert_eq!(base_digest.len(), FINGERPRINT_LENGTH);
    Ok(())
}
