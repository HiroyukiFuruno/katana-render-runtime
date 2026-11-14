#[cfg(all(unix, not(target_os = "macos")))]
use super::record_actual_selected_face;
use super::{MODE, Mode, ROUTE, active, rasterize_normally, rasterize_with_audit};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
#[cfg(all(unix, not(target_os = "macos")))]
use crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT;
#[cfg(all(unix, not(target_os = "macos")))]
use resvg::usvg::fontdb::Database;
use std::cell::Cell;
#[cfg(unix)]
use std::{ffi::OsString, fs, os::unix::ffi::OsStringExt};

const AUDIT_SENTINEL: u8 = 17;

#[test]
fn passive_source_audit_preserves_normal_raster_route_and_pixels() -> Result<(), String> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="32"><text x="2" y="24" font-family="Arial" font-size="20">actual route</text></svg>"#;
    let baseline = rasterize_normally(svg)?;
    let (audited, audit) = rasterize_with_audit(svg, assert_audit_callback_state)?;
    assert_normal_route_is_restored();
    assert_same_raster(&baseline, &audited?);
    assert_audit_contains_font_evidence(&audit);
    assert_final_glyph_evidence(&audit, svg)?;
    Ok(())
}

#[test]
fn malformed_svg_audit_propagates_render_error_and_restores_mode() -> Result<(), String> {
    let (rendered, audit) = rasterize_with_audit("<svg", assert_audit_callback_state)?;
    assert!(rendered.is_err());
    assert_eq!(audit["route"], "actual-current-route");
    assert_normal_route_is_restored();
    assert!(matches!(MODE.with(Cell::get), Mode::Measure));
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn non_utf8_actual_file_path_is_rejected_by_macos_filesystem() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = temp
        .path()
        .join(OsString::from_vec(b"font-\xff.ttf".to_vec()));
    let error = fs::write(&path, b"font")
        .err()
        .ok_or("macOS accepted non-UTF8 path")?;
    assert_eq!(error.raw_os_error(), Some(nix::libc::EILSEQ));
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn non_utf8_actual_file_audit_propagates_json_error_and_restores_route() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = temp
        .path()
        .join(OsString::from_vec(b"font-\xff.ttf".to_vec()));
    fs::write(&path, BUNDLED_SANS_SERIF_FONT).map_err(|error| error.to_string())?;
    let mut database = Database::new();
    database
        .load_font_file(&path)
        .map_err(|error| error.to_string())?;
    let id = database
        .faces()
        .next()
        .map(|face| face.id)
        .ok_or("non-UTF-8 font path was not loaded into fontdb")?;

    let error = SvgRasterizeOps::with_actual_font_source_audit(|| {
        assert_audit_callback_state();
        record_actual_selected_face(&database, Some(id));
        AUDIT_SENTINEL
    })
    .err()
    .ok_or("non-UTF-8 FaceSource path unexpectedly serialized")?;

    assert_eq!(error.to_string(), "path contains invalid UTF-8 characters");
    assert_normal_route_is_restored();
    assert!(matches!(MODE.with(Cell::get), Mode::Measure));
    Ok(())
}

#[test]
fn empty_actual_font_audit_serializes_and_restores_the_measure_route() -> Result<(), String> {
    let (result, audit) = SvgRasterizeOps::with_actual_font_source_audit(|| {
        assert_audit_callback_state();
        AUDIT_SENTINEL
    })
    .map_err(|error| error.to_string())?;

    assert_eq!(result, AUDIT_SENTINEL);
    assert_eq!(audit["route"], "actual-current-route");
    assert_eq!(audit["faces"], serde_json::json!([]));
    assert_eq!(audit["errors"], serde_json::json!([]));
    assert_normal_route_is_restored();
    assert!(matches!(MODE.with(Cell::get), Mode::Measure));
    Ok(())
}

fn assert_audit_callback_state() {
    assert!(!active());
    assert_eq!(ROUTE.with(Cell::get), None);
    assert!(matches!(MODE.with(Cell::get), Mode::Audit));
}

fn assert_normal_route_is_restored() {
    assert!(!active());
    assert_eq!(ROUTE.with(Cell::get), None);
}

fn assert_same_raster(
    baseline: &crate::markdown::svg_rasterize::RasterizedSvg,
    audited: &crate::markdown::svg_rasterize::RasterizedSvg,
) {
    assert_eq!(
        (baseline.width, baseline.height),
        (audited.width, audited.height)
    );
    assert_eq!(baseline.rgba, audited.rgba);
}

fn assert_audit_contains_font_evidence(audit: &serde_json::Value) {
    assert!(
        audit["faces"]
            .as_array()
            .is_some_and(|faces| !faces.is_empty())
            || audit["errors"]
                .as_array()
                .is_some_and(|errors| !errors.is_empty())
    );
    assert_eq!(audit["dropped"], 0);
}

fn assert_final_glyph_evidence(audit: &serde_json::Value, svg: &str) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let expected_sha = Sha256::digest(svg.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let trees = audit["finalGlyphTrees"]
        .as_array()
        .ok_or_else(|| "final glyph tree evidence is absent".to_owned())?;
    assert_eq!(trees.len(), 1);
    assert_eq!(trees[0]["paintCall"], 1);
    assert_eq!(trees[0]["treePath"], "root");
    assert_eq!(trees[0]["sourceSvgSha256"], expected_sha);
    assert!(
        trees[0]["faces"]
            .as_array()
            .is_some_and(|faces| !faces.is_empty())
    );
    assert!(audit["finalGlyphErrors"].is_array());
    assert_eq!(audit["finalGlyphDropped"], 0);
    Ok(())
}
