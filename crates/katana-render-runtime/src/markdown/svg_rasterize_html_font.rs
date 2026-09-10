#[path = "svg_rasterize_html_font_request.rs"]
mod request;
#[path = "svg_rasterize_html_font_sources.rs"]
mod sources;

use request::HtmlFontRequest;
use resvg::usvg;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock},
};

const MAX_CACHED_HTML_FONT_DATABASES: usize = 8;

struct HtmlFontDatabaseCache {
    entries: VecDeque<(HtmlFontRequest, Arc<usvg::fontdb::Database>)>,
}

impl HtmlFontDatabaseCache {
    fn get_or_load(&mut self, request: HtmlFontRequest) -> Arc<usvg::fontdb::Database> {
        if let Some(index) = self
            .entries
            .iter()
            .position(|(cached_request, _)| cached_request == &request)
            && let Some(entry) = self.entries.remove(index)
        {
            let database = Arc::clone(&entry.1);
            self.entries.push_back(entry);
            return database;
        }

        let database = Arc::new(sources::build_html_font_db(&request));
        self.entries.push_back((request, Arc::clone(&database)));
        if self.entries.len() > MAX_CACHED_HTML_FONT_DATABASES {
            self.entries.pop_front();
        }
        database
    }
}

pub(in super::super) fn html_rasterizer_options(markup: &str) -> usvg::Options<'static> {
    super::rasterizer_options_with_font_db(html_font_db_for_markup(markup))
}

pub(in super::super) fn html_font_db_for_markup(markup: &str) -> Arc<usvg::fontdb::Database> {
    html_font_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_or_load(HtmlFontRequest::from_markup(markup))
}

pub(in super::super) fn html_font_db_for_text(
    font_family: &str,
    text: &str,
) -> Arc<usvg::fontdb::Database> {
    html_font_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_or_load(HtmlFontRequest::from_text(font_family, text))
}

fn html_font_cache() -> &'static Mutex<HtmlFontDatabaseCache> {
    static CACHE: OnceLock<Mutex<HtmlFontDatabaseCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(HtmlFontDatabaseCache {
            entries: VecDeque::new(),
        })
    })
}

#[cfg(test)]
#[derive(Debug)]
pub(in super::super) struct HtmlFontMemorySnapshot {
    pub(in super::super) owned_font_bytes: usize,
    pub(in super::super) system_font_file_faces: usize,
    pub(in super::super) total_faces: usize,
}

#[cfg(test)]
pub(in super::super) fn html_font_memory_snapshot(
    database: &usvg::fontdb::Database,
) -> HtmlFontMemorySnapshot {
    let mut binary_sources = std::collections::HashSet::new();
    let mut owned_font_bytes = 0;
    let mut system_font_file_faces = 0;
    for face in database.faces() {
        if let usvg::fontdb::Source::Binary(data) = &face.source {
            let identity = Arc::as_ptr(data) as *const () as usize;
            if binary_sources.insert(identity) {
                owned_font_bytes += data.as_ref().as_ref().len();
            }
        } else {
            system_font_file_faces += 1;
        }
    }
    HtmlFontMemorySnapshot {
        owned_font_bytes,
        system_font_file_faces,
        total_faces: database.len(),
    }
}

#[cfg(test)]
pub(in super::super) fn current_process_rss_kib() -> Option<usize> {
    let process_id = std::process::id().to_string();
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &process_id])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then_some(output.stdout)
        .and_then(|stdout| String::from_utf8(stdout).ok())?
        .trim()
        .parse()
        .ok()
}
