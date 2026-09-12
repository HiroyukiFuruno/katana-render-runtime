#[path = "svg_rasterize_html_font_entities.rs"]
mod entities;
#[path = "svg_rasterize_html_font_request.rs"]
mod request;
#[path = "svg_rasterize_html_font_sources.rs"]
mod sources;
#[path = "svg_rasterize_html_font_system.rs"]
mod system;

#[cfg(test)]
#[path = "svg_rasterize_html_font_request_tests.rs"]
mod request_tests;

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
    html_font_db_for_request(html_font_cache(), HtmlFontRequest::from_markup(markup))
}

pub(in super::super) fn html_font_db_for_text(
    font_family: &str,
    text: &str,
) -> Arc<usvg::fontdb::Database> {
    html_font_db_for_request(
        html_font_cache(),
        HtmlFontRequest::from_text(font_family, text),
    )
}

fn html_font_db_for_request(
    cache: &Mutex<HtmlFontDatabaseCache>,
    request: HtmlFontRequest,
) -> Arc<usvg::fontdb::Database> {
    get_or_load_from_lock(cache.lock(), request)
}

fn get_or_load_from_lock(
    lock: std::sync::LockResult<std::sync::MutexGuard<'_, HtmlFontDatabaseCache>>,
    request: HtmlFontRequest,
) -> Arc<usvg::fontdb::Database> {
    lock.unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_or_load(request)
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
    parse_process_rss(
        std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &process_id])
            .output(),
    )
}

#[cfg(test)]
fn parse_process_rss(output: std::io::Result<std::process::Output>) -> Option<usize> {
    let output = match output {
        Ok(output) => output,
        Err(_) => return None,
    };
    if !output.status.success() {
        return None;
    }
    let stdout = match String::from_utf8(output.stdout) {
        Ok(stdout) => stdout,
        Err(_) => return None,
    };
    stdout.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{HtmlFontDatabaseCache, get_or_load_from_lock, parse_process_rss};
    use crate::markdown::svg_rasterize::font::html::request::HtmlFontRequest;
    use std::collections::VecDeque;
    use std::process::Command;
    use std::sync::Mutex;

    #[test]
    fn process_rss_parser_rejects_command_failures_and_invalid_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let command_failure = Command::new("definitely-not-a-real-command-for-krr").output();
        assert!(parse_process_rss(command_failure).is_none());

        let invalid_utf8 = Command::new("sh").args(["-c", "printf '\\377'"]).output()?;
        assert!(parse_process_rss(Ok(invalid_utf8)).is_none());

        let nonzero_status = Command::new("sh").args(["-c", "exit 1"]).output()?;
        assert!(parse_process_rss(Ok(nonzero_status)).is_none());
        Ok(())
    }

    #[test]
    fn font_database_helper_recovers_a_poisoned_local_cache()
    -> Result<(), Box<dyn std::error::Error>> {
        let cache = Mutex::new(HtmlFontDatabaseCache {
            entries: VecDeque::new(),
        });
        let guard = cache
            .lock()
            .map_err(|_| "fresh local cache must not be poisoned")?;
        let poisoned = Err(std::sync::PoisonError::new(guard));
        let database = get_or_load_from_lock(
            poisoned,
            HtmlFontRequest::from_text("Noto Sans", "cache recovery"),
        );
        assert!(database.faces().next().is_some());
        Ok(())
    }
}
