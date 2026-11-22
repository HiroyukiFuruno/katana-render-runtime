use resvg::usvg::{self, FontVariation, GlyphId, fontdb};
use std::{cell::RefCell, sync::Arc};

#[path = "svg_rasterize_native_html_face_outline_registry.rs"]
mod outlines;
use outlines::OutlineInstances;
pub(super) use outlines::{OutlineInstance, same_variations};

const ALIAS_PREFIX: &str = "__krr_native_html_face_";
const MAX_FACES: usize = 16;
const SYSTEM_ROOTS: [&str; 2] = ["/System/Library/", "/Library/Apple/System/Library/"];

struct RegisteredFace {
    alias: String,
    face: fontdb::FaceInfo,
    outlines: OutlineInstances,
}

thread_local! {
    static REGISTRY: RefCell<Option<Vec<RegisteredFace>>> = const { RefCell::new(None) };
}

pub(in crate::markdown::svg_rasterize) struct Scope {
    previous: Option<Vec<RegisteredFace>>,
}

impl Scope {
    pub(in crate::markdown::svg_rasterize) fn enter() -> Self {
        let previous = REGISTRY.with(|registry| registry.replace(Some(Vec::new())));
        Self { previous }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        REGISTRY.with(|registry| {
            let _ = registry.replace(self.previous.take());
        });
    }
}

pub(super) fn register(
    face: &fontdb::FaceInfo,
    outline: Option<OutlineInstance>,
) -> Option<String> {
    canonical_system_face(face)?;
    REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        let faces = registry.as_mut()?;
        let entry = registered_face(faces, face)?;
        if let Some(outline) = outline {
            entry.outlines.insert(outline)?;
        }
        Some(entry.alias.clone())
    })
}

fn registered_face<'a>(
    faces: &'a mut Vec<RegisteredFace>,
    face: &fontdb::FaceInfo,
) -> Option<&'a mut RegisteredFace> {
    if let Some(index) = faces
        .iter()
        .position(|entry| same_source(&entry.face, face))
    {
        return faces.get_mut(index);
    }
    if faces.len() >= MAX_FACES {
        return None;
    }
    let alias = format!("{ALIAS_PREFIX}{}", faces.len());
    faces.push(RegisteredFace {
        alias,
        face: face.clone(),
        outlines: OutlineInstances::default(),
    });
    faces.last_mut()
}

pub(in crate::markdown::svg_rasterize) fn apply(
    markup: &str,
    database: &Arc<fontdb::Database>,
) -> Arc<fontdb::Database> {
    let used = REGISTRY.with(|registry| {
        let registry = registry.borrow();
        registry
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter(|entry| {
                markup.contains(&format!("font-family=\"&quot;{}&quot;\"", entry.alias))
            })
            .map(aliased_face)
            .collect::<Vec<_>>()
    });
    if used.is_empty() {
        return Arc::clone(database);
    }
    let mut cloned = database.as_ref().clone();
    for face in used {
        cloned.push_face_info(face);
    }
    Arc::new(cloned)
}

fn aliased_face(entry: &RegisteredFace) -> fontdb::FaceInfo {
    let mut face = entry.face.clone();
    face.id = fontdb::ID::dummy();
    face.families = vec![(entry.alias.clone(), fontdb::Language::English_UnitedStates)];
    face
}

pub(in crate::markdown::svg_rasterize) fn select_alias(
    font: &usvg::Font,
    database: &fontdb::Database,
) -> Option<Option<fontdb::ID>> {
    for family in font.families() {
        let usvg::FontFamily::Named(name) = family else {
            continue;
        };
        if !name.starts_with(ALIAS_PREFIX) {
            continue;
        }
        let mut matches = database
            .faces()
            .filter(|face| face.families.iter().any(|(family, _)| family == name))
            .map(|face| face.id);
        let first = matches.next();
        return Some(first.filter(|_| matches.next().is_none()));
    }
    None
}

fn canonical_system_face(face: &fontdb::FaceInfo) -> Option<()> {
    let path = match &face.source {
        fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => path,
        fontdb::Source::Binary(_) => return None,
    };
    let canonical = std::fs::canonicalize(path).ok()?;
    (canonical == *path && SYSTEM_ROOTS.iter().any(|root| path.starts_with(root))).then_some(())
}

fn same_source(left: &fontdb::FaceInfo, right: &fontdb::FaceInfo) -> bool {
    left.index == right.index
        && left.post_script_name == right.post_script_name
        && source_path(left) == source_path(right)
}

pub(super) fn glyph_outline(
    database: &fontdb::Database,
    id: fontdb::ID,
    glyph: GlyphId,
    variations: &[FontVariation],
) -> Option<tiny_skia_path::Path> {
    let face = database.face(id)?;
    let (family, _) = face.families.first()?;
    if !family.starts_with(ALIAS_PREFIX) {
        return None;
    }
    REGISTRY.with(|registry| {
        let registry = registry.borrow();
        let entry = registry
            .as_ref()?
            .iter()
            .find(|entry| entry.alias == *family && same_source(&entry.face, face))?;
        entry
            .outlines
            .path(u16::try_from(glyph.0).ok()?, variations)
    })
}

fn source_path(face: &fontdb::FaceInfo) -> Option<&std::path::Path> {
    match &face.source {
        fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => Some(path),
        fontdb::Source::Binary(_) => None,
    }
}
