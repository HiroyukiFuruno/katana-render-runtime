use super as sources;
use super::super::shared_raster::shared_sources::{SharedSourceSeed, SharedSources};
use super::super::{proof::CreationProof, request::HtmlFontRequest};
use resvg::usvg::fontdb::Database;

#[cfg(test)]
pub(in super::super) fn build(request: &HtmlFontRequest) -> (Database, CreationProof) {
    build_with_shared_budget(request, 0)
}

#[cfg(test)]
pub(in super::super) fn build_with_shared_budget(
    request: &HtmlFontRequest,
    shared_budget: usize,
) -> (Database, CreationProof) {
    build_with_shared_sources(request, shared_budget, &[])
}

pub(in super::super) fn build_with_shared_sources(
    request: &HtmlFontRequest,
    shared_budget: usize,
    seeds: &[SharedSourceSeed],
) -> (Database, CreationProof) {
    let mut database = Database::new();
    let mut proof = CreationProof::default();
    let mut shared_sources = SharedSources::with_seeds(seeds);
    let mut remaining_bytes = shared_budget;
    database.load_font_source(sources::bundled_font_source());
    load_requested_sources(
        &mut database,
        &mut proof,
        request,
        &mut shared_sources,
        &mut remaining_bytes,
    );
    proof.capture_prefix(&database);
    load_fallbacks(
        &mut database,
        &mut proof,
        request,
        &mut shared_sources,
        &mut remaining_bytes,
    );
    sources::normalize_macos_hiragino_weights(&mut database);
    sources::configure_html_generic_families(&mut database);
    (database, proof)
}

fn load_requested_sources(
    database: &mut Database,
    proof: &mut CreationProof,
    request: &HtmlFontRequest,
    shared_sources: &mut SharedSources,
    remaining_bytes: &mut usize,
) {
    load_requested_families(database, proof, request, shared_sources, remaining_bytes);
    if sources::interface_bold_cjk_request(request) {
        load_paths(
            database,
            proof,
            sources::SYSTEM_CJK_FONT_PATHS,
            shared_sources,
            remaining_bytes,
        );
        sources::normalize_macos_hiragino_weights(database);
        let _ = sources::register_interface_bold(database);
    }
    sources::normalize_macos_hiragino_weights(database);
}

fn load_requested_families(
    database: &mut Database,
    proof: &mut CreationProof,
    request: &HtmlFontRequest,
    shared_sources: &mut SharedSources,
    remaining_bytes: &mut usize,
) {
    for family in &request.families {
        if family == "sans-serif" {
            load_sans_serif_sources(database, proof, shared_sources, remaining_bytes);
        } else {
            load_paths(
                database,
                proof,
                sources::paths_for_font_family(family),
                shared_sources,
                remaining_bytes,
            );
        }
    }
}

fn load_sans_serif_sources(
    database: &mut Database,
    proof: &mut CreationProof,
    shared_sources: &mut SharedSources,
    remaining_bytes: &mut usize,
) {
    let configured = sources::configured_html_sans_serif_family().to_ascii_lowercase();
    load_paths(
        database,
        proof,
        sources::paths_for_font_family(&configured),
        shared_sources,
        remaining_bytes,
    );
    load_paths(
        database,
        proof,
        sources::ARIAL_FONT_PATHS,
        shared_sources,
        remaining_bytes,
    );
    load_paths(
        database,
        proof,
        sources::LIBERATION_SANS_FONT_PATHS,
        shared_sources,
        remaining_bytes,
    );
}

fn load_paths(
    database: &mut Database,
    proof: &mut CreationProof,
    paths: &[&str],
    shared_sources: &mut SharedSources,
    remaining_bytes: &mut usize,
) {
    proof.load_shared(database, paths, shared_sources, remaining_bytes);
}

fn load_fallbacks(
    database: &mut Database,
    proof: &mut CreationProof,
    request: &HtmlFontRequest,
    shared_sources: &mut SharedSources,
    remaining_bytes: &mut usize,
) {
    load_cjk_fallback(database, proof, request, shared_sources, remaining_bytes);
    if request.needs_emoji {
        load_paths(
            database,
            proof,
            sources::EMOJI_FONT_PATHS,
            shared_sources,
            remaining_bytes,
        );
    }
    if request.needs_unicode_fallback {
        load_paths(
            database,
            proof,
            sources::UNICODE_FALLBACK_FONT_PATHS,
            shared_sources,
            remaining_bytes,
        );
    }
}

fn load_cjk_fallback(
    database: &mut Database,
    proof: &mut CreationProof,
    request: &HtmlFontRequest,
    shared_sources: &mut SharedSources,
    remaining_bytes: &mut usize,
) {
    if request.needs_cjk {
        load_paths(
            database,
            proof,
            &sources::cjk_fallback_font_paths(request),
            shared_sources,
            remaining_bytes,
        );
    }
}
