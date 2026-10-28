use super::super::admission;
#[cfg(test)]
use super::super::raster_observer;
use resvg::usvg;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) fn install(
    options: &mut usvg::Options<'static>,
    proof: Arc<super::super::proof::CreationProof>,
    text: String,
    valid: Arc<AtomicBool>,
    selected: Arc<AtomicBool>,
) {
    let selector = std::mem::replace(
        &mut options.font_resolver.select_font,
        usvg::FontResolver::default_font_selector(),
    );
    let font_valid = Arc::clone(&valid);
    options.font_resolver.select_font = Box::new(move |font, database| {
        let actual = selector(font, database);
        selected.store(true, Ordering::Relaxed);
        let admitted = selector_admitted(font, database, &proof, &text, actual).is_some();
        #[cfg(test)]
        raster_observer::known_selector(font, database, actual, admitted);
        if !admitted {
            font_valid.store(false, Ordering::Relaxed);
        }
        actual
    });
    install_fallback(options, valid);
}

pub(super) fn install_fallback(options: &mut usvg::Options<'static>, valid: Arc<AtomicBool>) {
    if let Some(styled) = options.font_resolver.select_styled_fallback.take() {
        let styled_valid = Arc::clone(&valid);
        options.font_resolver.select_styled_fallback =
            Some(Box::new(move |font, character, excluded, database| {
                styled_valid.store(false, Ordering::Relaxed);
                let actual = styled(font, character, excluded, database);
                #[cfg(test)]
                raster_observer::known_fallback(character, excluded, database, actual);
                actual
            }));
    }
    let fallback = std::mem::replace(
        &mut options.font_resolver.select_fallback,
        usvg::FontResolver::default_fallback_selector(),
    );
    options.font_resolver.select_fallback = Box::new(move |character, excluded, database| {
        valid.store(false, Ordering::Relaxed);
        let actual = fallback(character, excluded, database);
        #[cfg(test)]
        raster_observer::known_fallback(character, excluded, database, actual);
        actual
    });
}

pub(super) fn selector_admitted<'a>(
    font: &usvg::Font,
    database: &'a Arc<usvg::fontdb::Database>,
    proof: &'a super::super::proof::CreationProof,
    text: &str,
    actual: Option<usvg::fontdb::ID>,
) -> Option<admission::Admission<'a>> {
    let Some(usvg::FontFamily::Named(first)) = font.families().first() else {
        #[cfg(test)]
        raster_observer::reject_family();
        return None;
    };
    if font.stretch() != usvg::FontStretch::Normal {
        #[cfg(test)]
        raster_observer::reject_stretch();
        return None;
    }
    let italic = admitted_italic(font)?;
    let expected = admission::admit_face(database, proof, first, text, font.weight(), italic);
    #[cfg(test)]
    raster_observer::admission_result(
        database,
        expected.as_ref().map(admission::Admission::selected),
        actual,
    );
    expected.filter(|expected| actual == Some(expected.selected()))
}

fn admitted_italic(font: &usvg::Font) -> Option<bool> {
    match font.style() {
        usvg::FontStyle::Normal => Some(false),
        usvg::FontStyle::Italic => Some(true),
        _ => {
            #[cfg(test)]
            raster_observer::reject_style();
            None
        }
    }
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_selector_boundary_tests.rs"]
mod boundary_tests;
