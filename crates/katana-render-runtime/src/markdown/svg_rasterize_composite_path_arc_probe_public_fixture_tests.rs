use super::{Request, read_request_at, run_probe, sha256};

const PROBE_CANVAS_WIDTH: u32 = 32;
const PROBE_CANVAS_HEIGHT: u32 = 24;
const AFFINE_TRANSFORM_COMPONENT_COUNT: usize = 6;
const RECTANGLE_BOUNDS_COMPONENT_COUNT: usize = 4;

const PUBLIC_ARC_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="24"><path d="M 4 12 A 8 8 0 0 1 20 12 L 4 12 Z" fill="#123456"/></svg>"##;
const PUBLIC_ARC_PATH_DATA: &str = "M 4 12 A 8 8 0 0 1 20 12 L 4 12 Z";
const PUBLIC_ARC_SVG_SHA256: &str =
    "effe8d400f3c6871f996a9de4aa9df3a5c50ab48d14ab252efcb713de32ba4e0";
const PUBLIC_ARC_PATH_SHA256: &str =
    "580015bbdf009cfb7c230fdef159e6e09396438c06786be018c617fcb8f90995";

#[test]
fn public_fixed_arc_svg_produces_isolated_artifacts() -> Result<(), String> {
    let directory =
        crate::markdown::svg_rasterize::probe_test_support::TempProbeDirectory::create()
            .map_err(|error| error.to_string())?;
    let request = public_fixture_request(&directory)?;
    run_probe(&request)?;
    assert_artifact_report(&request)
}

fn public_fixture_request(
    directory: &crate::markdown::svg_rasterize::probe_test_support::TempProbeDirectory,
) -> Result<Request, String> {
    let svg = directory.file("public-arc.svg");
    let path_data = directory.file("public-arc-path.txt");
    let request_path = directory.file("public-arc-request.json");
    let output = directory.file("public-arc-output");
    std::fs::write(&svg, PUBLIC_ARC_SVG).map_err(|error| error.to_string())?;
    std::fs::write(&path_data, PUBLIC_ARC_PATH_DATA).map_err(|error| error.to_string())?;
    let request_json = serde_json::json!({
        "svg": svg,
        "path_data": path_data,
        "output": output,
        "expected_svg_sha256": PUBLIC_ARC_SVG_SHA256,
        "expected_path_sha256": PUBLIC_ARC_PATH_SHA256,
    });
    let request_bytes = serde_json::to_vec(&request_json).map_err(|error| error.to_string())?;
    std::fs::write(&request_path, request_bytes).map_err(|error| error.to_string())?;
    read_request_at(&request_path)
}

fn assert_artifact_report(request: &Request) -> Result<(), String> {
    let report_path = request.output.join("report.json");
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    assert_eq!(report["svgSha256"], request.expected_svg_sha256);
    assert_eq!(report["pathSha256"], request.expected_path_sha256);
    for (name, key) in [
        ("cubic.png", "cubicPngSha256"),
        ("conic.png", "conicPngSha256"),
        ("cubic-white.png", "whiteCubicPngSha256"),
        ("conic-white.png", "whiteConicPngSha256"),
    ] {
        assert_eq!(
            report[key],
            assert_png(&request.output.join(name))?,
            "{name} SHA256"
        );
    }
    assert_geometry_report(&report);
    assert_resvg_report(request)
}

fn assert_geometry_report(report: &serde_json::Value) {
    for (key, expected_count) in [
        ("transform", AFFINE_TRANSFORM_COMPONENT_COUNT),
        ("cubicBounds", RECTANGLE_BOUNDS_COMPONENT_COUNT),
        ("conicBounds", RECTANGLE_BOUNDS_COMPONENT_COUNT),
    ] {
        assert!(
            report[key].as_array().is_some_and(|values| {
                values.len() == expected_count
                    && values
                        .iter()
                        .all(|value| value.as_f64().is_some_and(f64::is_finite))
            }),
            "{key} must contain finite geometry"
        );
    }
}

fn assert_resvg_report(request: &Request) -> Result<(), String> {
    let report_path = request.output.join("resvg-report.json");
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    assert_eq!(
        report["pngSha256"],
        assert_png(&request.output.join("resvg.png"))?
    );
    assert_eq!(report["svgSha256"], request.expected_svg_sha256);
    assert_eq!(report["pathSha256"], request.expected_path_sha256);
    Ok(())
}

fn assert_png(path: &std::path::Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    assert!(!bytes.is_empty(), "{} is empty", path.display());
    let decoded = image::load_from_memory(&bytes).map_err(|error| error.to_string())?;
    assert_eq!(decoded.width(), PROBE_CANVAS_WIDTH);
    assert_eq!(decoded.height(), PROBE_CANVAS_HEIGHT);
    assert!(
        decoded.to_rgba8().pixels().any(|pixel| {
            pixel[3] != 0 && (pixel[0] != 255 || pixel[1] != 255 || pixel[2] != 255)
        })
    );
    Ok(sha256(&bytes))
}
