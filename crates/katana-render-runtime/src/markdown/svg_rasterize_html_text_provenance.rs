#[cfg(any(target_os = "macos", test))]
use resvg::usvg;
#[cfg(any(target_os = "macos", test))]
use sha2::{Digest, Sha256};
#[cfg(any(target_os = "macos", test))]
#[path = "svg_rasterize_html_text_provenance_routing.rs"]
mod routing;
#[cfg(any(target_os = "macos", test))]
pub(crate) use routing::HtmlTextRouting;
#[cfg(any(target_os = "macos", test))]
#[path = "svg_rasterize_html_text_provenance_tree.rs"]
mod tree;
#[cfg(test)]
use tree::tree_ids;

#[cfg(any(target_os = "macos", test))]
pub(crate) type ParsedHtmlDocument<'input> =
    Result<usvg::roxmltree::Document<'input>, usvg::roxmltree::Error>;

#[derive(Clone, Default)]
pub(crate) struct GeneratedHtmlTextProvenance {
    #[cfg(any(target_os = "macos", test))]
    entries: Vec<GeneratedHtmlTextEntry>,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Clone)]
struct GeneratedHtmlTextEntry {
    id: String,
    markup_sha256: String,
}

impl GeneratedHtmlTextProvenance {
    pub(crate) fn record(&mut self, markup: String) -> String {
        #[cfg(not(any(target_os = "macos", test)))]
        {
            let _ = self;
            return markup;
        }
        #[cfg(any(target_os = "macos", test))]
        return self.record_text_markup(markup);
    }

    #[cfg(any(target_os = "macos", test))]
    fn record_text_markup(&mut self, markup: String) -> String {
        if !markup.starts_with("<text ") {
            return markup;
        }
        let Some(open_end) = markup.find('>') else {
            return markup;
        };
        let start_tag = &markup[..open_end];
        if start_tag.contains(" id=") {
            return markup;
        }
        let id = format!("krr-html-text-{}", self.entries.len() + 1);
        let marked = format!("{start_tag} id=\"{id}\"{}", &markup[open_end..]);
        self.register_marked_text(id, marked)
    }

    #[cfg(any(target_os = "macos", test))]
    fn register_marked_text(&mut self, id: String, markup: String) -> String {
        self.entries.push(GeneratedHtmlTextEntry {
            id,
            markup_sha256: sha256(markup.as_bytes()),
        });
        markup
    }
}

#[cfg(any(target_os = "macos", test))]
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "svg_rasterize_html_text_provenance_tests.rs"]
mod tests;
