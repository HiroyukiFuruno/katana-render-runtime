use super::*;
use crate::markdown::svg_rasterize::{SvgRasterizeOps, font::html::proof::CreationProof};
use resvg::usvg::fontdb::{Database, FaceInfo, ID, Source};
use std::{path::PathBuf, sync::atomic::AtomicU64};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const ASCII_TEXT: &str = "AB";
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

struct FileFixture {
    path: PathBuf,
    database: Arc<Database>,
    proof: Arc<CreationProof>,
    id: ID,
    family: String,
}

impl FileFixture {
    fn new() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!(
            "krr-selector-boundary-{}-{}.ttf",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        let result = Self::load(path.clone(), &mut file);
        if result.is_err() {
            std::fs::remove_file(path).map_err(|error| error.to_string())?;
        }
        result
    }

    fn load(path: PathBuf, file: &mut std::fs::File) -> Result<Self, String> {
        std::io::Write::write_all(file, FONT_BYTES).map_err(|error| error.to_string())?;
        let mut database = Database::new();
        let mut proof = CreationProof::default();
        proof.load(
            &mut database,
            &[path.to_str().ok_or("fixture path is not UTF-8")?],
        );
        assert_eq!(database.len(), 1);
        let (id, family) = real_file_identity(&database, &path)?;
        database.set_sans_serif_family(family.clone());
        proof.capture_prefix(&database);
        assert!(proof.current(&database));
        Ok(Self {
            path,
            database: Arc::new(database),
            proof: Arc::new(proof),
            id,
            family,
        })
    }
}

fn real_file_identity(database: &Database, path: &std::path::Path) -> Result<(ID, String), String> {
    let face = database
        .faces()
        .next()
        .ok_or("real Noto File face missing")?;
    assert!(matches!(&face.source, Source::File(actual) if actual == path));
    let family = face
        .families
        .first()
        .ok_or("real File family missing")?
        .0
        .clone();
    Ok((face.id, family))
}

impl Drop for FileFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

struct Case {
    family: String,
    attribute: &'static str,
    first_family: String,
    style: &'static str,
    stretch: &'static str,
    reason: Option<&'static str>,
}

fn named_case(
    fixture: &FileFixture,
    attribute: &'static str,
    style: &'static str,
    stretch: &'static str,
    reason: Option<&'static str>,
) -> Case {
    Case {
        family: fixture.family.clone(),
        attribute,
        first_family: format!("Some(Named({:?}))", fixture.family),
        style,
        stretch,
        reason,
    }
}

fn selection_cases(fixture: &FileFixture) -> [Case; 4] {
    [
        named_case(fixture, "", "Normal", "Normal", None),
        named_case(
            fixture,
            "font-stretch='condensed'",
            "Normal",
            "Condensed",
            Some("stretch_not_normal"),
        ),
        named_case(
            fixture,
            "font-style='oblique'",
            "Oblique",
            "Normal",
            Some("style_not_normal_or_italic"),
        ),
        Case {
            family: "sans-serif".into(),
            attribute: "",
            first_family: "Some(SansSerif)".into(),
            style: "Normal",
            stretch: "Normal",
            reason: Some("first_family_not_named_or_missing"),
        },
    ]
}

fn case_svg(case: &Case) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><text y="20" font-family="{}" {}>{ASCII_TEXT}</text></svg>"#,
        case.family, case.attribute
    )
}

fn parse_case(fixture: &FileFixture, case: &Case) -> Result<raster_observer::Snapshot, String> {
    let mut options = usvg::Options {
        fontdb: Arc::clone(&fixture.database),
        ..Default::default()
    };
    let valid = Arc::new(AtomicBool::new(true));
    let selected = Arc::new(AtomicBool::new(false));
    install(
        &mut options,
        Arc::clone(&fixture.proof),
        ASCII_TEXT.into(),
        Arc::clone(&valid),
        Arc::clone(&selected),
    );
    let scope = raster_observer::Scope::enter();
    let tree =
        usvg::Tree::from_str(&case_svg(case), &options).map_err(|error| error.to_string())?;
    assert!(selected.load(Ordering::Relaxed));
    assert_eq!(valid.load(Ordering::Relaxed), case.reason.is_none());
    assert!(!tree.root().children().is_empty());
    assert_tree_prefix(fixture, tree.fontdb());
    drop(tree);
    drop(options);
    let capture = scope.snapshot();
    assert_eq!(capture.dropped, 0);
    Ok(capture)
}

fn assert_tree_prefix(fixture: &FileFixture, database: &Arc<Database>) {
    assert_eq!(database.len(), fixture.database.len());
    assert!(fixture.proof.current(database));
    assert!(
        fixture
            .database
            .faces()
            .zip(database.faces())
            .all(|(left, right)| {
                crate::markdown::svg_rasterize::font::html::proof::same_face(left, right)
            })
    );
}

fn assert_requested_font(event: &serde_json::Value, case: &Case) {
    assert_eq!(event["route"], "known_probe");
    assert_eq!(event["firstFamily"], case.first_family);
    assert_eq!(
        event["families"]
            .as_array()
            .and_then(|v| v.first())
            .and_then(serde_json::Value::as_str),
        case.first_family
            .strip_prefix("Some(")
            .and_then(|value| value.strip_suffix(')'))
    );
    assert_eq!(event["style"], case.style);
    assert_eq!(event["stretch"], case.stretch);
    assert_eq!(event["admitted"], case.reason.is_none());
}

fn assert_actual_file(event: &serde_json::Value, fixture: &FileFixture) -> Result<(), String> {
    let face: &FaceInfo = fixture
        .database
        .face(fixture.id)
        .ok_or("expected File face missing")?;
    let actual = &event["actualFace"];
    assert_eq!(event["weight"], face.weight.0);
    assert_eq!(
        actual["databaseFaceIdLocalOnly"],
        format!("{:?}", fixture.id)
    );
    assert_eq!(actual["sourceKind"], "File");
    assert_eq!(actual["sourcePath"], fixture.path.display().to_string());
    assert_eq!(actual["faceIndex"], face.index);
    assert_eq!(actual["postScriptName"], face.post_script_name);
    Ok(())
}

fn assert_rejections(capture: &raster_observer::Snapshot, case: &Case, selectors: usize) {
    let rejected: Vec<_> = capture
        .events
        .iter()
        .filter(|event| event["stage"] == "admission_rejection")
        .collect();
    if let Some(reason) = case.reason {
        assert_eq!(rejected.len(), selectors);
        assert!(rejected.iter().all(|event| event["reason"] == reason));
    } else {
        assert!(rejected.is_empty());
        assert_named_admission_accepted(capture);
    }
}

fn assert_named_admission_accepted(capture: &raster_observer::Snapshot) {
    let reasons: Vec<_> = capture
        .events
        .iter()
        .filter(|event| event["stage"] == "admission_result")
        .map(|event| &event["reason"])
        .collect();
    assert!(!reasons.is_empty());
    assert!(reasons.iter().all(|reason| *reason == "accepted"));
}

fn assert_case(
    capture: &raster_observer::Snapshot,
    fixture: &FileFixture,
    case: &Case,
) -> Result<(), String> {
    let selectors: Vec<_> = capture
        .events
        .iter()
        .filter(|event| event["stage"] == "selector")
        .collect();
    assert!(!selectors.is_empty());
    for event in &selectors {
        assert_requested_font(event, case);
        assert_actual_file(event, fixture)?;
    }
    assert_rejections(capture, case, selectors.len());
    assert!(
        !capture
            .events
            .iter()
            .any(|event| event["stage"] == "fallback")
    );
    assert!(fixture.proof.current(&fixture.database));
    Ok(())
}

#[test]
fn official_usvg_condensed_oblique_and_generic_selection_rejects_known_admission()
-> Result<(), String> {
    /* WHY: Fontを手作りせず公式parser/default selectorの実要求と実File選択を検証する。 */
    let fixture = FileFixture::new()?;
    let owner = Arc::downgrade(&fixture.database);
    SvgRasterizeOps::with_html_font_render_scope(|| -> Result<(), String> {
        for case in selection_cases(&fixture) {
            let capture = parse_case(&fixture, &case)?;
            assert_case(&capture, &fixture, &case)?;
        }
        Ok(())
    })?;
    assert_eq!(Arc::strong_count(&fixture.database), 1);
    drop(fixture);
    assert!(owner.upgrade().is_none());
    Ok(())
}
