use super::family_runs::candidates::specified_family_faces;
use super::family_runs::resolved_font_runs;
use super::font::matching_font_face;
use super::font::platform_fallback::actual_fallback::NativeHtmlFallbackBase;
use super::html_font_db_for_measured_text;
use crate::markdown::svg_rasterize::{HtmlFontStyle, HtmlPaintFontRun, font, native_html_faces};
use resvg::usvg::fontdb::{Database, FaceInfo, ID, Source};
use std::{path::Path, sync::Arc};

#[path = "svg_rasterize_text_native_html_run_memo.rs"]
mod memo;
#[path = "svg_rasterize_text_native_html_run_metrics.rs"]
mod metrics;
pub(super) use memo::Scope;
use memo::{Memo, SelectedFace, SelectionKey};

enum RunFace {
    Existing(ID),
    Native(Arc<SelectedFace>),
}

struct Run {
    face: RunFace,
    text: String,
}

pub(in crate::markdown::svg_rasterize) struct RunPlan {
    database: Arc<Database>,
    runs: Vec<Run>,
}

impl RunPlan {
    pub(in crate::markdown::svg_rasterize) fn build(
        text: &str,
        style: HtmlFontStyle<'_>,
    ) -> Option<Self> {
        if style.italic || !text.chars().any(font::is_cjk_character) {
            return None;
        }
        let database =
            html_font_db_for_measured_text(style.family, text, style.weight, style.italic);
        let base = matching_font_face(&database, style.family, style.weight, style.italic)?;
        let specified = specified_family_faces(&database, style.family, style.weight, style.italic);
        let original = resolved_font_runs(
            &database,
            base,
            style.family,
            text,
            style.weight,
            style.italic,
        );
        let query = SelectionQuery::new(&database, base, style)?;
        let runs = memo::with_current(|memo| query.runs(memo, original, &specified))?;
        runs.iter()
            .any(|run| matches!(run.face, RunFace::Native(_)))
            .then_some(Self { database, runs })
    }

    pub(in crate::markdown::svg_rasterize) fn paint_runs(&self) -> Option<Vec<HtmlPaintFontRun>> {
        self.runs.iter().map(|run| self.paint_run(run)).collect()
    }

    fn paint_run(&self, run: &Run) -> Option<HtmlPaintFontRun> {
        let (family, variations) = match &run.face {
            RunFace::Existing(id) => (
                self.database.face(*id)?.families.first()?.0.clone(),
                Vec::new(),
            ),
            RunFace::Native(selected) => (selected.alias.clone(), selected.variations.clone()),
        };
        Some(HtmlPaintFontRun {
            family,
            text: run.text.clone(),
            variations,
        })
    }
}

struct SelectionQuery<'a> {
    database: &'a Database,
    base: ID,
    system_ui: bool,
    key: SelectionKey,
}

impl<'a> SelectionQuery<'a> {
    fn new(database: &'a Database, base: ID, style: HtmlFontStyle<'_>) -> Option<Self> {
        let face = database.face(base)?;
        let system_ui = font::is_system_ui_primary(face);
        let base_name = if system_ui {
            "System Font".into()
        } else {
            format!("{}:{}", source_path(face)?.display(), face.index)
        };
        Some(Self {
            database,
            base,
            system_ui,
            key: SelectionKey {
                base: base_name,
                size: super::effective_html_font_size(style.size).to_bits(),
                weight: style.weight,
                language: style.language.map(str::to_owned),
            },
        })
    }

    fn runs(&self, memo: &mut Memo, original: Vec<(ID, String)>, specified: &[ID]) -> Vec<Run> {
        let mut runs = Vec::<Run>::new();
        for (old_id, text) in original {
            for ch in text.chars() {
                let selected = self.select(memo, ch, old_id, specified);
                if let Some(last) = runs
                    .last_mut()
                    .filter(|last| same_run_face(&last.face, &selected))
                {
                    last.text.push(ch);
                } else {
                    runs.push(Run {
                        face: selected,
                        text: ch.to_string(),
                    });
                }
            }
        }
        runs
    }

    fn select(&self, memo: &mut Memo, ch: char, old_id: ID, specified: &[ID]) -> RunFace {
        if !font::is_cjk_character(ch) || specified.contains(&old_id) {
            return RunFace::Existing(old_id);
        }
        let Some(selected) = memo.selection(self, ch) else {
            return RunFace::Existing(old_id);
        };
        let old = self.database.face(old_id);
        let new = selected.database.face(selected.id);
        if selected.variations.is_empty()
            && matches!((old, new), (Some(a), Some(b)) if a.index == b.index && source_path(a) == source_path(b))
        {
            return RunFace::Existing(old_id);
        }
        RunFace::Native(selected)
    }

    fn native_base(&self) -> NativeHtmlFallbackBase<'_> {
        if self.system_ui {
            NativeHtmlFallbackBase::SystemUi
        } else {
            NativeHtmlFallbackBase::FileBackedSelectedFace {
                database: self.database,
                id: self.base,
            }
        }
    }
}

fn same_run_face(left: &RunFace, right: &RunFace) -> bool {
    match (left, right) {
        (RunFace::Existing(a), RunFace::Existing(b)) => a == b,
        (RunFace::Native(a), RunFace::Native(b)) => {
            a.id == b.id
                && Arc::ptr_eq(&a.database, &b.database)
                && native_html_faces::same_variations(&a.variations, &b.variations)
        }
        _ => false,
    }
}

fn source_path(face: &FaceInfo) -> Option<&Path> {
    match &face.source {
        Source::File(path) | Source::SharedFile(path, _) => Some(path),
        Source::Binary(_) => None,
    }
}

#[cfg(test)]
#[path = "svg_rasterize_text_native_html_runs_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_text_native_html_run_test_support.rs"]
mod test_support;
