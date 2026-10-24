#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_font_lazy_diagnostic.rs"]
mod lazy_diagnostic;
#[cfg(all(test, target_os = "macos"))]
pub(in crate::markdown::svg_rasterize) use lazy_diagnostic::record_actual_selected_face;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_lazy_parse.rs"]
mod lazy_parse;
#[cfg(target_os = "macos")]
pub(in super::super) use lazy_parse::rasterize as rasterize_html_known_or_eager;
#[path = "svg_rasterize_html_font_entities.rs"]
mod entities;
#[path = "svg_rasterize_html_font_request.rs"]
mod request;
#[path = "svg_rasterize_html_font_sources.rs"]
mod sources;
#[cfg(target_os = "macos")]
pub(in crate::markdown::svg_rasterize) use sources::{
    bold_alias_id, has_native_interface_candidate, is_interface_face, is_internal_family,
    is_system_ui_primary,
};
#[path = "svg_rasterize_html_font_system.rs"]
mod system;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_known_admission.rs"]
mod admission;
#[path = "svg_rasterize_html_font_discovery.rs"]
mod discovery;
#[path = "svg_rasterize_html_font_discovery_cache.rs"]
mod discovery_cache;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_creation_proof.rs"]
mod proof;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_shared_raster.rs"]
mod shared_raster;
use discovery::CacheValue;
pub(in crate::markdown::svg_rasterize) use discovery::{
    DatabaseKey, DiscoveryMode, MeasuredDatabase,
};
pub(in super::super) use discovery_cache::{html_font_db_for_measured_text, measured_database};
#[cfg(all(test, target_os = "macos"))]
pub(in crate::markdown::svg_rasterize) use shared_raster::render_html_with_options;
#[cfg(test)]
#[path = "svg_rasterize_html_font_request_tests.rs"]
mod request_tests;

pub(in super::super) use request::HtmlFontRequest;
use resvg::usvg;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock},
};

const MAX_CACHED_HTML_FONT_DATABASES: usize = 8;

struct HtmlFontDatabaseCache {
    entries: VecDeque<(DatabaseKey, CacheValue)>,
}

impl HtmlFontDatabaseCache {
    fn get_or_load(&mut self, request: HtmlFontRequest) -> Arc<usvg::fontdb::Database> {
        self.get_mode(DatabaseKey::eager(request)).database
    }
}

pub(in super::super) fn html_rasterizer_options(markup: &str) -> usvg::Options<'static> {
    let mut options = super::rasterizer_options_with_font_db(html_font_db_for_markup(markup));
    super::super::text_shaping::install_html_font_resolver(&mut options);
    options
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

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_font_decision_observer.rs"]
mod decision_observer;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_font_raster_observer.rs"]
pub(in crate::markdown::svg_rasterize) mod raster_observer;

#[cfg(test)]
mod diagnostic_binary_fixture {
    #[cfg(target_os = "macos")]
    type RealDatabase = (
        std::sync::Arc<resvg::usvg::fontdb::Database>,
        resvg::usvg::fontdb::ID,
        std::sync::Weak<dyn AsRef<[u8]> + Send + Sync>,
    );

    #[cfg(target_os = "macos")]
    pub(super) fn real_database(source_error: &'static str) -> Result<RealDatabase, String> {
        let mut database = resvg::usvg::fontdb::Database::new();
        database.load_font_data(super::super::BUNDLED_SANS_SERIF_FONT.to_vec());
        let face = database.faces().next().ok_or("real Noto face missing")?;
        let id = face.id;
        let resvg::usvg::fontdb::Source::Binary(bytes) = &face.source else {
            return Err(source_error.into());
        };
        let bytes = std::sync::Arc::downgrade(bytes);
        assert_eq!(
            database.with_face_data(id, |data, _| {
                data == super::super::BUNDLED_SANS_SERIF_FONT
            }),
            Some(true)
        );
        Ok((std::sync::Arc::new(database), id, bytes))
    }
}
