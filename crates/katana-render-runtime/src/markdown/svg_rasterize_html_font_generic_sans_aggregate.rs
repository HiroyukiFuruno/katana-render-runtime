use super::super::super::super::{HtmlFontRequest, sources};
use super::face_assertions::{diagnostic_face_data, diagnostic_face_report};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, text_shaping};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

const NORMAL_FONT_WEIGHT: u16 = 400;
const FONT_SIZE_PX: u16 = 16;
const AGGREGATE_WITH_FIVE_TRAILING_LFS: &str = "\nOpen linked page\nToggle panel\nSubmit\n\n\n\n\n";
const AGGREGATE_WITH_THREE_TRAILING_LFS: &str = "\nOpen linked page\nToggle panel\nSubmit\n\n\n";

type DiagnosticResolvedRuns = (ID, Vec<(ID, String)>);
type DiagnosticRunReports = (
    Vec<serde_json::Value>,
    std::collections::HashSet<ID>,
    f32,
    f32,
);

fn diagnostic_resolved_runs(
    database: &Arc<Database>,
    family: &str,
    text: &str,
) -> Result<DiagnosticResolvedRuns, String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let base = text_shaping::known_matching(database, family, NORMAL_FONT_WEIGHT, false)?;
        let runs = text_shaping::diagnostic_resolved_font_runs(
            database,
            base,
            family,
            text,
            NORMAL_FONT_WEIGHT,
            false,
        );
        Some((base, runs))
    })
    .ok_or_else(|| "could not resolve the diagnostic base face and runs".to_string())
}

fn diagnostic_run_reports(
    database: &Database,
    runs: Vec<(ID, String)>,
) -> Result<DiagnosticRunReports, String> {
    let mut run_reports = Vec::new();
    let mut measured = std::collections::HashSet::new();
    let mut ascent = 0.0_f32;
    let mut descent = 0.0_f32;
    for (face_id, run_text) in runs {
        let report = diagnostic_face_report(database, face_id)?;
        let (lf_supported, (run_ascent, run_descent)) = diagnostic_face_data(database, face_id)
            .ok_or("resolved run face could not be parsed")?;
        if measured.insert(face_id) {
            ascent = ascent.max(run_ascent);
            descent = descent.max(run_descent);
        }
        run_reports.push(serde_json::json!({"text": run_text, "face": report, "runFaceSupportsLineFeed": lf_supported}));
    }
    Ok((run_reports, measured, ascent, descent))
}

fn diagnostic_lf_coverage(
    database: &Database,
) -> Result<(Vec<serde_json::Value>, usize, usize), String> {
    let mut lf_faces = Vec::new();
    let mut inspected_face_count = 0usize;
    for face in database.faces() {
        match diagnostic_face_data(database, face.id) {
            Some((true, _)) => {
                inspected_face_count += 1;
                lf_faces.push(diagnostic_face_report(database, face.id)?);
            }
            Some((false, _)) => inspected_face_count += 1,
            None => {}
        }
    }
    let face_count = database.faces().count();
    Ok((lf_faces, inspected_face_count, face_count))
}

fn aggregate_probe_report(
    database_kind: &str,
    family: &str,
    text: &str,
    base_report: serde_json::Value,
    run_reports: Vec<serde_json::Value>,
    metrics: (f32, f32),
    coverage: (Vec<serde_json::Value>, usize, usize),
) -> serde_json::Value {
    let (ascent, descent) = metrics;
    let (lf_faces, inspected_face_count, face_count) = coverage;
    serde_json::json!({
        "databaseKind": database_kind, "requestedFamily": family, "text": text,
        "fontSize": FONT_SIZE_PX, "weight": NORMAL_FONT_WEIGHT, "italic": false, "baseFace": base_report,
        "resolvedRuns": run_reports, "combinedLineMetrics": { "ascent": ascent, "descent": descent },
        "lineFeedFaceCoverage": { "faceCount": face_count, "inspectableFaceCount": inspected_face_count,
            "uninspectableFaceCount": face_count - inspected_face_count, "facesSupportingLineFeed": lf_faces },
    })
}

fn print_aggregate_probe(
    database_kind: &str,
    database: &Arc<Database>,
    family: &str,
    text: &str,
) -> Result<(), String> {
    let (base, runs) = diagnostic_resolved_runs(database, family, text)?;
    let base_report = diagnostic_face_report(database, base)?;
    let (run_reports, _measured, ascent, descent) = diagnostic_run_reports(database, runs)?;
    let coverage = diagnostic_lf_coverage(database)?;
    println!(
        "KRR_AGGREGATE_FONT_PROBE {}",
        aggregate_probe_report(
            database_kind,
            family,
            text,
            base_report,
            run_reports,
            (ascent, descent),
            coverage
        )
    );
    Ok(())
}

pub(in super::super) fn check_aggregate_faces_and_metrics() -> Result<(), String> {
    let cases = [
        AGGREGATE_WITH_FIVE_TRAILING_LFS,
        AGGREGATE_WITH_THREE_TRAILING_LFS,
    ];
    for text in cases {
        for family in ["sans-serif", "Arial"] {
            let request = HtmlFontRequest::from_text(family, text);
            let (known, _proof) = sources::build_known_html_font_db(&request);
            let known = Arc::new(known);
            let eager = Arc::new(sources::build_html_font_db(&request));
            for (database_kind, database) in [("known", &known), ("eager", &eager)] {
                print_aggregate_probe(database_kind, database, family, text)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Database, diagnostic_lf_coverage};
    use resvg::usvg::fontdb::Source;

    #[test]
    fn font_coverage_counts_registered_face_with_missing_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let missing = directory.path().join("missing.ttf");
        let retained = directory.path().join("retained.ttf");
        let moved = directory.path().join("moved.ttf");
        let bytes = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
        std::fs::write(&missing, bytes)?;
        std::fs::write(&retained, bytes)?;
        let mut database = Database::new();
        assert_eq!(
            database
                .load_font_source(Source::File(missing.clone()))
                .len(),
            1
        );
        assert_eq!(database.load_font_source(Source::File(retained)).len(), 1);
        std::fs::rename(&missing, &moved)?;
        let (_, inspected, total) = diagnostic_lf_coverage(&database)?;
        assert_eq!((inspected, total), (1, 2));
        std::fs::rename(moved, missing)?;
        let (_, inspected, total) = diagnostic_lf_coverage(&database)?;
        assert_eq!((inspected, total), (2, 2));
        Ok(())
    }
}
