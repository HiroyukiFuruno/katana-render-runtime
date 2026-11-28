use super::super::SelectedFont;
use super::{PhysicalFace, ResolvedFont, physical_file};
use core_foundation::{array::CFArray, base::TCFType, url::CFURL};
use core_text::{
    font_descriptor::CTFontDescriptor, font_manager::CTFontManagerCreateFontDescriptorsFromURL,
};
use resvg::usvg::fontdb::Database;
use std::{fs, path::Path};

#[path = "identity_coretext_variations.rs"]
mod variations;

#[path = "identity_coretext_candidates.rs"]
mod candidates;

pub(super) fn resolve_font(
    database: &Database,
    selected: &SelectedFont,
) -> Result<ResolvedFont, String> {
    let candidates = candidates::candidate_files(database, selected)?;
    let mut matches = Vec::new();
    for (path, identity) in candidates {
        matches.extend(resolve_file(&path, identity, &selected.post_script_name)?);
    }
    match matches.as_slice() {
        [resolved] => Ok(resolved.clone()),
        [] => Err("no exact URL-bound CoreText descriptor matched the CDP PostScript name and exposed a variable instance".to_string()),
        _ => Err("multiple URL-bound CoreText physical face matches are ambiguous".to_string()),
    }
}

pub(super) fn verify_single_font_data(bytes: &[u8], face_index: u32) -> Result<(), String> {
    candidates::verify_single_font_data(bytes, face_index)
}

fn resolve_file(
    path: &Path,
    identity: PhysicalFace,
    selected_postscript_name: &str,
) -> Result<Vec<ResolvedFont>, String> {
    verify_candidate_identity(
        path,
        &identity,
        "physical candidate changed before CoreText descriptor lookup",
    )?;
    let descriptors = descriptors_from_url(path)?;
    let mut matches = Vec::new();
    for descriptor in &descriptors {
        if variations::descriptor_postscript_name(&descriptor)? != selected_postscript_name {
            continue;
        }
        matches.push(resolve_descriptor(
            &descriptor,
            path,
            &identity,
            selected_postscript_name,
        )?);
    }
    verify_candidate_after_lookup(path, &identity)?;
    Ok(matches)
}

fn verify_candidate_after_lookup(path: &Path, identity: &PhysicalFace) -> Result<(), String> {
    verify_candidate_identity(
        path,
        identity,
        "physical font changed during CoreText descriptor lookup",
    )
}

fn verify_candidate_identity(
    path: &Path,
    identity: &PhysicalFace,
    mismatch_error: &str,
) -> Result<(), String> {
    let (actual_path, digest, _) = physical_file(path)?;
    if actual_path != identity.path || digest != identity.file_sha256 {
        return Err(mismatch_error.to_owned());
    }
    Ok(())
}

fn descriptors_from_url(path: &Path) -> Result<CFArray<CTFontDescriptor>, String> {
    let url = CFURL::from_path(path, false).ok_or_else(|| {
        "CoreText candidate path could not be represented as a file URL".to_string()
    })?;
    /* SAFETY: CoreTextのCreate関数が返す所有済み配列を、URLが有効な間にnull確認して所有権ラッパーへ渡す。 */
    let descriptors =
        unsafe { CTFontManagerCreateFontDescriptorsFromURL(url.as_concrete_TypeRef()) };
    if descriptors.is_null() {
        return Err("CoreText URL descriptor lookup returned null".to_string());
    }
    /* SAFETY: CTFontManagerCreateFontDescriptorsFromURLは保持済みのdescriptor配列を返す。 */
    Ok(unsafe { CFArray::wrap_under_create_rule(descriptors) })
}

fn resolve_descriptor(
    descriptor: &CTFontDescriptor,
    query_path: &Path,
    identity: &PhysicalFace,
    selected_postscript_name: &str,
) -> Result<ResolvedFont, String> {
    let descriptor_path =
        verify_descriptor_binding(descriptor, query_path, identity, selected_postscript_name)?;
    variations::verify_ctfont_descriptor_name(descriptor, selected_postscript_name)?;
    Ok(ResolvedFont {
        identity: identity.clone(),
        method: "URL-bound CoreText named-face descriptor metadata; coordinates are not Chrome instance evidence",
        core_text_descriptor: Some(variations::evidence(&descriptor_path, descriptor)?),
    })
}

fn verify_descriptor_binding(
    descriptor: &CTFontDescriptor,
    query_path: &Path,
    identity: &PhysicalFace,
    selected_postscript_name: &str,
) -> Result<String, String> {
    let descriptor_path = descriptor
        .font_path()
        .ok_or_else(|| "exact CoreText descriptor has no physical URL".to_string())?;
    verify_descriptor_path(&descriptor_path, query_path, identity)?;
    verify_descriptor_name(descriptor, selected_postscript_name)?;
    descriptor_path_text(&descriptor_path)
}

fn descriptor_path_text(descriptor_path: &Path) -> Result<String, String> {
    descriptor_path
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| "exact CoreText descriptor URL is not valid UTF-8".to_string())
}

fn verify_descriptor_path(
    descriptor_path: &Path,
    query_path: &Path,
    identity: &PhysicalFace,
) -> Result<(), String> {
    let canonical_path = fs::canonicalize(descriptor_path)
        .map_err(|error| format!("canonicalize CoreText descriptor URL: {error}"))?;
    if canonical_path != identity.path || canonical_path != query_path {
        return Err("exact CoreText descriptor URL differs from its query file".to_string());
    }
    Ok(())
}

fn verify_descriptor_name(
    descriptor: &CTFontDescriptor,
    selected_postscript_name: &str,
) -> Result<(), String> {
    let descriptor_name = variations::descriptor_postscript_name(descriptor)?;
    if descriptor_name != selected_postscript_name {
        return Err("CoreText descriptor PostScript name differs from CDP".to_string());
    }
    Ok(())
}

#[cfg(test)]
#[path = "identity_coretext_binding_tests.rs"]
mod binding_tests;

#[cfg(test)]
#[path = "identity_coretext_lookup_consistency_tests.rs"]
mod lookup_consistency_tests;
