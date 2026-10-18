use super::super::proof::CreationProof;
use resvg::usvg::fontdb::Database;
use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{Arc, Weak},
};

#[path = "svg_rasterize_html_font_local_missing_fixture_support.rs"]
mod fixture_support;

type ProvenDatabase = (Arc<Database>, Arc<CreationProof>);
type WeakBytes = Weak<dyn AsRef<[u8]> + Send + Sync>;

enum Mutation {
    Missing(PathBuf),
    Replace {
        selected: PathBuf,
        replacement: PathBuf,
    },
}

struct Fixture {
    pending: Option<ProvenDatabase>,
    mutation: Mutation,
    before_acquisition: bool,
    mapped: Vec<WeakBytes>,
    mutated: bool,
    mutation_error: Option<String>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
}

pub(in super::super) struct Scope {
    previous: Option<Fixture>,
}

impl Scope {
    pub(in super::super) fn enter(
        database: Arc<Database>,
        proof: Arc<CreationProof>,
        missing: PathBuf,
    ) -> Result<Self, String> {
        fixture_support::require_absent(&missing)?;
        Self::local(database, proof, Mutation::Missing(missing), false)
    }

    pub(in super::super) fn before_acquisition(
        database: Arc<Database>,
        proof: Arc<CreationProof>,
        missing: PathBuf,
    ) -> Result<Self, String> {
        fixture_support::require_absent(&missing)?;
        Self::local(database, proof, Mutation::Missing(missing), true)
    }

    pub(in super::super) fn replace_after_admission(
        database: Arc<Database>,
        proof: Arc<CreationProof>,
        selected: PathBuf,
        replacement: PathBuf,
    ) -> Result<Self, String> {
        Self::local(
            database,
            proof,
            Mutation::Replace {
                selected,
                replacement,
            },
            true,
        )
    }

    fn local(
        database: Arc<Database>,
        proof: Arc<CreationProof>,
        mutation: Mutation,
        before_acquisition: bool,
    ) -> Result<Self, String> {
        fixture_support::validate_fixture(&database, &mutation)?;
        let fixture = Fixture {
            pending: Some((database, proof)),
            mutation,
            before_acquisition,
            mapped: Vec::new(),
            mutated: false,
            mutation_error: None,
        };
        Ok(Self {
            previous: ACTIVE.with(|active| active.replace(Some(fixture))),
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

pub(in super::super) fn take_fixture(
    database: Arc<Database>,
    proof: Arc<CreationProof>,
) -> ProvenDatabase {
    ACTIVE
        .with(|active| {
            active
                .borrow_mut()
                .as_mut()
                .and_then(|fixture| fixture.pending.take())
        })
        .unwrap_or((database, proof))
}

pub(in super::super) fn after_admission() {
    fixture_support::after_admission();
}

pub(in super::super) fn after_selector(database: &Database, admitted: bool) {
    fixture_support::after_selector(database, admitted);
}
