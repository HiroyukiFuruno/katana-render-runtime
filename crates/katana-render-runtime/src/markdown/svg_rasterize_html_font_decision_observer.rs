use super::{DatabaseKey, MeasuredDatabase};
use resvg::usvg::fontdb::Database;
use std::{cell::RefCell, sync::Weak};

const MAX_DECISIONS: usize = 128;
const MAX_INPUT_BYTES: usize = 4096;
const MAX_REQUEST_FAMILIES: usize = 64;

#[derive(Clone)]
pub(super) struct Decision {
    pub(super) key: DatabaseKey,
    pub(super) family: String,
    pub(super) text: String,
    pub(super) weight: u16,
    pub(super) italic: bool,
    pub(super) database: Weak<Database>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Vec<Decision>>> = const { RefCell::new(None) };
}

pub(super) struct Scope {
    previous: Option<Vec<Decision>>,
}

impl Scope {
    pub(super) fn enter() -> Self {
        Self {
            previous: ACTIVE.with(|active| active.replace(Some(Vec::new()))),
        }
    }

    pub(super) fn records(&self) -> Vec<Decision> {
        ACTIVE.with(|active| match active.borrow().as_ref() {
            Some(records) => records.clone(),
            None => Vec::new(),
        })
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| {
            active.replace(self.previous.take());
        });
    }
}

pub(super) fn record(
    selected: &MeasuredDatabase,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(records) = active.as_mut() else {
            return;
        };
        if records.len() >= MAX_DECISIONS || !input_bounded(selected, family, text) {
            return;
        }
        records.push(Decision {
            key: selected.key.clone(),
            family: family.to_string(),
            text: text.to_string(),
            weight,
            italic,
            database: std::sync::Arc::downgrade(&selected.database),
        });
    });
}

fn input_bounded(selected: &MeasuredDatabase, family: &str, text: &str) -> bool {
    family
        .len()
        .checked_add(text.len())
        .is_some_and(|bytes| bytes <= MAX_INPUT_BYTES)
        && selected.key.request.families.len() <= MAX_REQUEST_FAMILIES
        && selected
            .key
            .request
            .families
            .iter()
            .map(String::len)
            .sum::<usize>()
            <= MAX_INPUT_BYTES
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_decision_observer_boundary_tests.rs"]
mod boundary_tests;
