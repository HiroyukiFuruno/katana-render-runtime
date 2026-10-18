use super as sources;
use super::super::{proof::CreationProof, request::HtmlFontRequest};
use resvg::usvg::fontdb::Database;

pub(in super::super) fn build(request: &HtmlFontRequest) -> (Database, CreationProof) {
    let mut database = Database::new();
    let mut proof = CreationProof::default();
    database.load_font_source(sources::bundled_font_source());
    for family in &request.families {
        if family == "sans-serif" {
            let configured = sources::configured_html_sans_serif_family().to_ascii_lowercase();
            proof.load(&mut database, sources::paths_for_font_family(&configured));
            proof.load(&mut database, sources::ARIAL_FONT_PATHS);
            proof.load(&mut database, sources::LIBERATION_SANS_FONT_PATHS);
        } else {
            proof.load(&mut database, sources::paths_for_font_family(family));
        }
    }
    proof.capture_prefix(&database);
    load_fallbacks(&mut database, &mut proof, request);
    sources::normalize_macos_hiragino_weights(&mut database);
    sources::configure_html_generic_families(&mut database);
    (database, proof)
}

fn load_fallbacks(database: &mut Database, proof: &mut CreationProof, request: &HtmlFontRequest) {
    if request.needs_cjk {
        proof.load(database, &sources::cjk_fallback_font_paths(request));
    }
    if request.needs_emoji {
        proof.load(database, sources::EMOJI_FONT_PATHS);
    }
    if request.needs_unicode_fallback {
        proof.load(database, sources::UNICODE_FALLBACK_FONT_PATHS);
    }
}
