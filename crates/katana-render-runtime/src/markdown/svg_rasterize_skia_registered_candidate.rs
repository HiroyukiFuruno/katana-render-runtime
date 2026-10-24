use std::collections::{BTreeMap, BTreeSet};

use resvg::usvg;
use sha2::{Digest, Sha256};
use skia_safe::{Data, FontMgr, Typeface, textlayout::TypefaceFontProvider};

use super::{RasterizedSvg, SvgRasterizeError};

type FaceKey = (String, u16, String, String);
type SourceIdentity = (Vec<u8>, u32);
type UsedFonts = (BTreeSet<usvg::fontdb::ID>, BTreeSet<String>);

pub(super) fn render(
    svg: &str,
    tree: &usvg::Tree,
    scale: f32,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    let font_mgr = registered_font_mgr(tree)?;
    super::skia_candidate::render_inner_with_font_mgr(svg, tree.size(), scale, false, font_mgr)
}

fn registered_font_mgr(tree: &usvg::Tree) -> Result<FontMgr, SvgRasterizeError> {
    let database = tree.fontdb();
    let system = FontMgr::default();
    let mut provider = TypefaceFontProvider::new();
    let mut registrations = BTreeMap::new();
    let (used_ids, requested_families) = used_face_ids(tree)?;
    for id in used_ids {
        register_face(database, id, &mut provider, &mut registrations, &system)?;
    }
    preflight_families(&requested_families, &provider)?;
    let mut ordered = skia_safe::utils::OrderedFontMgr::new();
    ordered.append(provider);
    ordered.append(system);
    Ok(ordered.into())
}

fn preflight_families(
    families: &BTreeSet<String>,
    provider: &TypefaceFontProvider,
) -> Result<(), SvgRasterizeError> {
    for family in families {
        if provider.match_family(family.as_str()).count() == 0 {
            return Err(SvgRasterizeError::RasterizeFailed(format!(
                "registered provider cannot resolve SVG family {family:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn used_face_ids(tree: &usvg::Tree) -> Result<UsedFonts, SvgRasterizeError> {
    let mut ids = BTreeSet::new();
    let mut families = BTreeSet::new();
    collect_group(tree.root(), &mut ids, &mut families)?;
    Ok((ids, families))
}

fn collect_group(
    group: &usvg::Group,
    ids: &mut BTreeSet<usvg::fontdb::ID>,
    families: &mut BTreeSet<String>,
) -> Result<(), SvgRasterizeError> {
    for node in group.children() {
        match node {
            usvg::Node::Group(child) => collect_group(child, ids, families)?,
            usvg::Node::Text(text) => {
                collect_text(text, ids, families)?;
            }
            _ => {}
        }
        let mut child_error = None;
        node.subroots(|subroot| {
            if child_error.is_none() {
                child_error = collect_group(subroot, ids, families).err();
            }
        });
        if let Some(error) = child_error {
            return Err(error);
        }
    }
    Ok(())
}

fn collect_text(
    text: &usvg::Text,
    ids: &mut BTreeSet<usvg::fontdb::ID>,
    families: &mut BTreeSet<String>,
) -> Result<(), SvgRasterizeError> {
    ids.extend(
        text.layouted()
            .iter()
            .flat_map(|span| span.positioned_glyphs.iter().map(|glyph| glyph.font)),
    );
    for span in text.chunks().iter().flat_map(|chunk| chunk.spans()) {
        /* WHY: SkiaのSVG parserはfamily一覧の先頭だけを検索する。 */
        let Some(usvg::FontFamily::Named(name)) = span.font().families().first() else {
            return Err(SvgRasterizeError::RasterizeFailed(
                "generic SVG font family is not supported by registered provider".to_string(),
            ));
        };
        families.insert(name.clone());
    }
    Ok(())
}

fn register_face(
    database: &usvg::fontdb::Database,
    id: usvg::fontdb::ID,
    provider: &mut TypefaceFontProvider,
    registrations: &mut BTreeMap<FaceKey, SourceIdentity>,
    system: &FontMgr,
) -> Result<(), SvgRasterizeError> {
    let face = database
        .face(id)
        .ok_or_else(|| error(id, "face metadata missing"))?;
    let families = face.families.iter().map(|(name, _)| name.as_str());
    let (typeface, source_identity) = load_typeface(database, id, system)?;
    for family in families {
        let key = family_key(family, face);
        if let Some(previous) = registrations.get(&key) {
            if previous != &source_identity {
                return Err(error(
                    id,
                    "family/style registration has conflicting physical faces",
                ));
            }
            continue;
        }
        registrations.insert(key, source_identity.clone());
        register_family_aliases(provider, &typeface, family);
    }
    Ok(())
}

fn register_family_aliases(provider: &mut TypefaceFontProvider, typeface: &Typeface, family: &str) {
    provider.register_typeface(typeface.clone(), Some(family));
    /* WHY: CSSのfamily名は大小文字を区別しないが、Skiaのproviderは区別する。 */
    let folded = family.to_lowercase();
    if folded != family {
        provider.register_typeface(typeface.clone(), Some(folded.as_str()));
    }
}

fn family_key(family: &str, face: &usvg::fontdb::FaceInfo) -> FaceKey {
    (
        family.to_lowercase(),
        face.weight.0,
        format!("{:?}", face.style),
        format!("{:?}", face.stretch),
    )
}

fn load_typeface(
    database: &usvg::fontdb::Database,
    id: usvg::fontdb::ID,
    system: &FontMgr,
) -> Result<(Typeface, SourceIdentity), SvgRasterizeError> {
    let (bytes, index) = database
        .with_face_data(id, |bytes, index| (bytes.to_vec(), index))
        .ok_or_else(|| error(id, "face bytes unavailable"))?;
    let identity = (Sha256::digest(&bytes).to_vec(), index);
    let typeface =
        super::physical_font_face::create_physical_typeface(&Data::new_copy(&bytes), index, system)
            .map_err(|message| error(id, &message))?
            .ok_or_else(|| error(id, "Skia could not create physical typeface"))?;
    verify_typeface(&typeface, &bytes, index, id)?;
    Ok((typeface, identity))
}

fn verify_typeface(
    typeface: &Typeface,
    source: &[u8],
    source_index: u32,
    id: usvg::fontdb::ID,
) -> Result<(), SvgRasterizeError> {
    let Some((cached, cached_index)) = typeface.to_existing_font_data() else {
        return Err(error(id, "registered typeface has no source data"));
    };
    if cached_index == source_index && cached.as_bytes() == source {
        return Ok(());
    }
    if source_index == 0 || cached_index != 0 {
        return Err(error(id, "registered typeface source identity differs"));
    }
    super::physical_font_face::verify_face(source, source_index, cached.as_bytes())
        .map_err(|message| error(id, &message))
}

fn error(id: usvg::fontdb::ID, message: &str) -> SvgRasterizeError {
    SvgRasterizeError::RasterizeFailed(format!("registered face {id:?}: {message}"))
}

#[path = "svg_rasterize_skia_registered_candidate_tests.rs"]
mod tests;
