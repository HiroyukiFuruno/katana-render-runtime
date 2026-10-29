use resvg::usvg;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tiny_skia::Pixmap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    svg: PathBuf,
    path_data: PathBuf,
    output: PathBuf,
    expected_svg_sha256: String,
    expected_path_sha256: String,
}

#[test]
#[ignore = "diagnostic only; isolated full-canvas paint differs from actual crop and clip"]
fn isolated_cubic_and_conic_paint_probe() -> Result<(), String> {
    let request = read_request()?;
    let svg = read_pinned(&request.svg, &request.expected_svg_sha256)?;
    let data = read_pinned(&request.path_data, &request.expected_path_sha256)?;
    if !svg.contains(data.trim()) {
        return Err("path data is absent from input".to_owned());
    }
    let tree = parse(&svg)?;
    let node = only_path(tree.root())?;
    if !node.is_visible() {
        return Err("drawable path is not visible".to_owned());
    }
    validate_path_data(node, &data)?;
    let mut prepared = super::prepare_path(node)?.ok_or("unsupported paint")?;
    let cubic_path = prepared.path.clone();
    let mut conic = skia_safe::Path::from_svg(data.trim()).ok_or("Skia SVG parse failed")?;
    conic = conic.with_fill_type(prepared.path.fill_type());
    if !conic.is_finite() || !conic.verbs().contains(&skia_safe::PathVerb::Conic) {
        return Err("finite conic path required".to_owned());
    }
    let (cubic, arc) = paint_pair(&tree, node, &mut prepared, conic.clone(), false)?;
    prepared.path = cubic_path.clone();
    let white = paint_pair(&tree, node, &mut prepared, conic, true)?;
    save_result(&request, node, &prepared, &cubic_path, &cubic, &arc, &white)
}

fn read_request() -> Result<Request, String> {
    let path = std::env::var_os("KRR_ISOLATED_ARC_PROBE_REQUEST")
        .map(PathBuf::from)
        .ok_or("diagnostic request path is required")?;
    if !path.is_absolute() {
        return Err("diagnostic request path must be absolute".to_owned());
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let request: Request = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if !request.svg.is_absolute()
        || !request.path_data.is_absolute()
        || !request.output.is_absolute()
    {
        return Err("input and output paths must be absolute".to_owned());
    }
    Ok(request)
}

fn read_pinned(path: &std::path::Path, expected: &str) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|error| error.to_string())?;
    if sha256(&data) != expected {
        return Err("diagnostic input SHA256 mismatch".to_owned());
    }
    String::from_utf8(data).map_err(|error| error.to_string())
}

fn parse(svg: &str) -> Result<usvg::Tree, String> {
    usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn only_path(group: &usvg::Group) -> Result<&usvg::Path, String> {
    let mut path = None;
    collect_path(group, &mut path)?;
    path.ok_or("drawable path is missing".to_owned())
}

fn collect_path<'a>(
    group: &'a usvg::Group,
    path: &mut Option<&'a usvg::Path>,
) -> Result<(), String> {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_path(group, path)?,
            usvg::Node::Path(found) if path.is_none() => *path = Some(found),
            _ => return Err("input must contain exactly one drawable path".to_owned()),
        }
    }
    Ok(())
}

fn validate_path_data(node: &usvg::Path, data: &str) -> Result<(), String> {
    let data = data.trim();
    if data.contains(['"', '<', '&', '\0']) {
        return Err("invalid path data characters".to_owned());
    }
    let source = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="{data}"/></svg>"#);
    let tree = parse(&source)?;
    let parsed = only_path(tree.root())?;
    if !node.data().segments().eq(parsed.data().segments()) {
        return Err("path data does not match drawable geometry".to_owned());
    }
    Ok(())
}

fn paint_pair(
    tree: &usvg::Tree,
    node: &usvg::Path,
    prepared: &mut super::PreparedPath,
    conic: skia_safe::Path,
    white_background: bool,
) -> Result<(Pixmap, Pixmap), String> {
    let size = tree.size().to_int_size();
    let mut cubic = Pixmap::new(size.width(), size.height()).ok_or("cubic allocation failed")?;
    let mut arc = Pixmap::new(size.width(), size.height()).ok_or("conic allocation failed")?;
    if white_background {
        cubic.fill(tiny_skia::Color::WHITE);
        arc.fill(tiny_skia::Color::WHITE);
    }
    let transform = node.abs_transform();
    if !super::transform_is_finite(transform) {
        return Err("non-finite transform".to_owned());
    }
    super::render_prepared_path(&mut cubic.as_mut(), prepared, transform, node.paint_order())?;
    prepared.path = conic;
    super::render_prepared_path(&mut arc.as_mut(), prepared, transform, node.paint_order())?;
    Ok((cubic, arc))
}

fn save_result(
    request: &Request,
    node: &usvg::Path,
    prepared: &super::PreparedPath,
    cubic_path: &skia_safe::Path,
    cubic: &Pixmap,
    arc: &Pixmap,
    white: &(Pixmap, Pixmap),
) -> Result<(), String> {
    std::fs::create_dir(&request.output).map_err(|error| error.to_string())?;
    let cubic_png = save_png(&request.output, "cubic.png", cubic)?;
    let arc_png = save_png(&request.output, "conic.png", arc)?;
    let white_cubic_png = save_png(&request.output, "cubic-white.png", &white.0)?;
    let white_arc_png = save_png(&request.output, "conic-white.png", &white.1)?;
    let size = (cubic.width(), cubic.height());
    let mut report = report(
        request, node, prepared, cubic_path, &cubic_png, &arc_png, size,
    );
    report["whiteCubicPngSha256"] = sha256(&white_cubic_png).into();
    report["whiteConicPngSha256"] = sha256(&white_arc_png).into();
    let bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    std::fs::write(request.output.join("report.json"), bytes).map_err(|error| error.to_string())
}

fn save_png(output: &std::path::Path, name: &str, pixmap: &Pixmap) -> Result<Vec<u8>, String> {
    let png = pixmap.encode_png().map_err(|error| error.to_string())?;
    std::fs::write(output.join(name), &png).map_err(|error| error.to_string())?;
    Ok(png)
}

fn report(
    request: &Request,
    node: &usvg::Path,
    prepared: &super::PreparedPath,
    cubic_path: &skia_safe::Path,
    cubic_png: &[u8],
    arc_png: &[u8],
    size: (u32, u32),
) -> serde_json::Value {
    let t = node.abs_transform();
    let bounds = prepared.path.bounds();
    let cubic_bounds = cubic_path.bounds();
    serde_json::json!({
        "diagnosticOnly": true, "fullCanvasWithoutCropOrClip": true,
        "svgSha256": request.expected_svg_sha256,
        "pathSha256": request.expected_path_sha256,
        "width": size.0, "height": size.1,
        "transform": [t.sx, t.ky, t.kx, t.sy, t.tx, t.ty],
        "fill": prepared.fill.is_some(), "stroke": prepared.stroke.is_some(),
        "cubicBounds": [cubic_bounds.left, cubic_bounds.top, cubic_bounds.right, cubic_bounds.bottom],
        "conicBounds": [bounds.left, bounds.top, bounds.right, bounds.bottom],
        "cubicVerbs": format!("{:?}", cubic_path.verbs()),
        "conicVerbs": format!("{:?}", prepared.path.verbs()),
        "cubicPngSha256": sha256(cubic_png), "conicPngSha256": sha256(arc_png),
    })
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
