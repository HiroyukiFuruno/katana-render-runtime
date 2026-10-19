use serde_json::Value;
use skia_safe::{
    Color, EncodedImageFormat, FontMgr, FontStyle, Size,
    font_style::{Slant, Weight, Width},
    svg::Dom,
    textlayout::TypefaceFontProvider,
};
use skrifa::{FontRef, raw::types::Tag};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;
#[path = "svg_rasterize_html_font_skia_tree_c4_diagnostic_support.rs"]
mod support;
use support::{VerifiedFace, svg_size, verified_faces};

fn registered_font_manager(faces: &[VerifiedFace]) -> Result<FontMgr, String> {
    let manager = FontMgr::default();
    let mut provider = TypefaceFontProvider::new();
    for face in faces {
        let typeface = select_verified_system_face(&manager, face)?;
        verify_sfnt_tables(&typeface, face)?;
        let family = typeface.family_name();
        let lowercase_family = family.to_lowercase();
        provider.register_typeface(typeface.clone(), Some(family.as_str()));
        provider.register_typeface(typeface, Some(lowercase_family.as_str()));
    }
    Ok(provider.into())
}

fn select_verified_system_face(
    manager: &FontMgr,
    face: &VerifiedFace,
) -> Result<skia_safe::Typeface, String> {
    let style = FontStyle::new(Weight::from(face.weight), Width::NORMAL, Slant::Upright);
    let typeface = manager
        .match_family_style(&face.family, style)
        .ok_or_else(|| format!("Skia system face unavailable family={}", face.family))?;
    if typeface.post_script_name().as_deref() != Some(face.post_script_name.as_str()) {
        return Err(format!(
            "Skia system PostScript mismatch path={}",
            face.path
        ));
    }
    Ok(typeface)
}

fn verify_sfnt_tables(typeface: &skia_safe::Typeface, face: &VerifiedFace) -> Result<(), String> {
    let source = FontRef::from_index(&face.bytes, face.index)
        .map_err(|error| format!("skrifa face parse failed: {error}"))?;
    let tags = typeface
        .read_table_tags()
        .ok_or("Skia system face table tags unavailable")?;
    verify_sfnt_tag_set(&source, &tags, &face.path)?;
    verify_sfnt_table_bytes(&source, typeface, &tags, &face.path)
}

fn verify_sfnt_tag_set(source: &FontRef<'_>, tags: &[u32], path: &str) -> Result<(), String> {
    let source_tags = source
        .table_directory()
        .table_records()
        .iter()
        .map(|record| u32::from_be_bytes(record.tag().to_be_bytes()))
        .collect::<BTreeSet<_>>();
    let skia_tags = tags.iter().copied().collect::<BTreeSet<_>>();
    if skia_tags.len() != tags.len() || skia_tags != source_tags {
        return Err(format!("SFNT table set differs path={path}"));
    }
    Ok(())
}

fn verify_sfnt_table_bytes(
    source: &FontRef<'_>,
    typeface: &skia_safe::Typeface,
    tags: &[u32],
    path: &str,
) -> Result<(), String> {
    for &tag in tags {
        let raw = source
            .table_data(Tag::from_u32(tag))
            .ok_or("physical SFNT table is absent")?;
        let skia = typeface
            .copy_table_data(tag)
            .ok_or("Skia SFNT table is absent")?;
        if raw.as_bytes() != skia.as_bytes() {
            return Err(format!("SFNT table bytes differ path={} tag={tag}", path));
        }
    }
    Ok(())
}

impl VerifiedFace {
    fn report(&self) -> Value {
        serde_json::json!({
            "path": self.path,
            "index": self.index,
            "bytesSha256": self.bytes_sha256,
            "postScriptName": self.post_script_name,
            "family": self.family,
            "weight": self.weight,
        })
    }
}

#[test]
#[ignore = "diagnostic only; requires a fresh HTML font audit request"]
fn ignored_standalone_svg_skia_tree_c4_diagnostic() -> Result<(), String> {
    let started = Instant::now();
    super::ignored_html_frame_and_font_audit_are_saved_together()?;
    let request = super::load_request()?;
    let audit = load_audit(&request.output)?;
    run_diagnostic(&request.output, started, &audit)
}

fn run_diagnostic(output: &Path, started: Instant, audit: &Value) -> Result<(), String> {
    let root = super::root_tree(audit)?;
    let (_source_svg, source_sha, paint_call) = super::root_render_input(audit)?;
    let (svg, svg_sha) = diagnostic_svg(root, output)?;
    let faces = verified_faces(root)?;
    let (width, height, container, view_box) = svg_size(&svg)?;
    let manager = registered_font_manager(&faces)?;
    let png_bytes = render_png(&svg, manager, container, width, height)?;
    let png_path = output.join("skia-standalone.svg.png");
    super::write_fresh(&png_path, &png_bytes)?;
    let binary = std::fs::read(std::env::current_exe().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let prepared = PreparedDiagnostic {
        source_sha,
        paint_call,
        svg,
        svg_sha,
        view_box,
        width,
        height,
        faces: faces.iter().map(VerifiedFace::report).collect(),
        binary_sha: super::super::super::output::sha256(&binary),
        png_path,
        png_sha: super::super::super::output::sha256(&png_bytes),
        png_size: png_bytes.len(),
    };
    write_prepared_report(output, started, prepared)
}

fn load_audit(output: &Path) -> Result<Value, String> {
    let report = std::fs::read(output.join("report.json")).map_err(|error| error.to_string())?;
    let report: Value = serde_json::from_slice(&report).map_err(|error| error.to_string())?;
    report
        .get("audit")
        .cloned()
        .ok_or_else(|| "audit payload is absent".to_owned())
}

struct AuditReport {
    source_sha: String,
    paint_call: u64,
    svg_sha: String,
    svg_size: usize,
    view_box: Value,
}

struct RenderReport {
    width: i32,
    height: i32,
    faces: Vec<Value>,
    binary_sha: String,
    png_path: std::path::PathBuf,
    png_sha: String,
    png_size: usize,
    elapsed: f64,
}

struct DiagnosticReport {
    audit: AuditReport,
    render: RenderReport,
}

struct PreparedDiagnostic {
    source_sha: String,
    paint_call: u64,
    svg: String,
    svg_sha: String,
    view_box: Value,
    width: i32,
    height: i32,
    faces: Vec<Value>,
    binary_sha: String,
    png_path: std::path::PathBuf,
    png_sha: String,
    png_size: usize,
}

fn write_prepared_report(
    output: &Path,
    started: Instant,
    prepared: PreparedDiagnostic,
) -> Result<(), String> {
    let report = DiagnosticReport {
        audit: AuditReport {
            source_sha: prepared.source_sha,
            paint_call: prepared.paint_call,
            svg_sha: prepared.svg_sha,
            svg_size: prepared.svg.len(),
            view_box: prepared.view_box,
        },
        render: RenderReport {
            width: prepared.width,
            height: prepared.height,
            faces: prepared.faces,
            binary_sha: prepared.binary_sha,
            png_path: prepared.png_path,
            png_sha: prepared.png_sha,
            png_size: prepared.png_size,
            elapsed: started.elapsed().as_secs_f64(),
        },
    };
    write_diagnostic_report(output, report)
}

fn write_diagnostic_report(output: &Path, report: DiagnosticReport) -> Result<(), String> {
    let output_value = report.value(output);
    let bytes = serde_json::to_vec_pretty(&output_value).map_err(|error| error.to_string())?;
    super::write_fresh(&output.join("skia-standalone-report.json"), &bytes)
}

impl DiagnosticReport {
    fn value(self, output: &Path) -> Value {
        serde_json::json!({
            "sourceSvgSha256": self.audit.source_sha,
            "sourceSvgPaintCall": self.audit.paint_call,
            "diagnosticSvgSha256": self.audit.svg_sha,
            "diagnosticSvgPath": output.join("skia-standalone-input.svg"),
            "diagnosticSvgSize": self.audit.svg_size,
            "viewBox": self.audit.view_box,
            "surface": {"width": self.render.width, "height": self.render.height, "alpha": "transparent"},
            "faces": self.render.faces,
            "binarySha256": self.render.binary_sha,
            "pngPath": self.render.png_path,
            "pngSha256": self.render.png_sha,
            "pngSize": self.render.png_size,
            "elapsedSeconds": self.render.elapsed,
            "formalAcceptance": false,
            "standaloneSvg": true,
            "htmlFramePaddingApplied": false,
        })
    }
}

fn render_png(
    svg: &str,
    manager: FontMgr,
    container: Size,
    width: i32,
    height: i32,
) -> Result<Vec<u8>, String> {
    let mut dom = Dom::from_bytes(svg.as_bytes(), manager)
        .map_err(|error| format!("Skia SVG parse failed: {error}"))?;
    dom.set_container_size(container);
    let mut surface = skia_safe::surfaces::raster_n32_premul((width, height))
        .ok_or("Skia raster surface creation failed")?;
    surface.canvas().clear(Color::TRANSPARENT);
    dom.render(surface.canvas());
    let png = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, 100)
        .ok_or("Skia PNG encoding failed")?;
    Ok(png.as_bytes().to_vec())
}

fn diagnostic_svg(root: &Value, output: &Path) -> Result<(String, String), String> {
    let svg = root["resolvedTextSvg"]
        .as_str()
        .ok_or("audited resolvedTextSvg is absent")?;
    let sha = super::super::super::output::sha256(svg.as_bytes());
    if root["resolvedTextSvgSha256"] != sha {
        return Err("diagnostic SVG SHA differs from audited tree".to_owned());
    }
    super::write_fresh(&output.join("skia-standalone-input.svg"), svg.as_bytes())?;
    Ok((svg.to_owned(), sha))
}
