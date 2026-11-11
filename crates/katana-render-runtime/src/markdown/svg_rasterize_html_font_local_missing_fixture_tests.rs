use super::super::proof::CreationProof;
use resvg::usvg::fontdb::{Database, Source};
use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{Arc, Weak},
};

#[path = "svg_rasterize_html_font_local_missing_fixture_support.rs"]
mod fixture_support;
#[path = "svg_rasterize_html_font_local_missing_source_fixture.rs"]
mod source_fixture;

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

#[test]
fn local_missing_source_guard_accepts_binary_and_rejects_transient_shared_file()
-> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let missing = temp.path().join("missing.ttf");
    let binary = source_fixture::binary_database();
    assert!(
        binary
            .faces()
            .any(|face| matches!(face.source, Source::Binary(_)))
    );
    let scope = Scope::enter(binary, Arc::new(CreationProof::default()), missing.clone())?;
    drop(scope);
    let (shared, shared_proof, _font_fixture) = source_fixture::transient_database(temp.path())?;
    assert!(
        shared
            .faces()
            .any(|face| matches!(face.source, Source::SharedFile(_, _)))
    );
    let error = Scope::enter(shared, shared_proof, missing)
        .err()
        .ok_or("shared font source unexpectedly accepted")?;
    assert_eq!(error, "shared font source is not fixture-local");
    Ok(())
}

#[test]
fn replacement_io_failure_preserves_selected_font() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let selected = directory.path().join("selected.ttf");
    let replacement = directory.path().join("replacement.ttf");
    let staging = selected.with_extension("admission-replacement");
    let selected_bytes = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
    let replacement_bytes = include_bytes!(
        "../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/MPLUS1p-Regular.ttf"
    );
    std::fs::write(&selected, selected_bytes)?;
    assert!(
        fixture_support::replace_selected_font(&selected, &replacement)
            .is_err_and(|error| error.starts_with("open copied public replacement font"))
    );
    assert_eq!(std::fs::read(&selected)?, selected_bytes.as_slice());
    assert!(!staging.try_exists()?);

    std::fs::write(&replacement, replacement_bytes)?;
    std::fs::write(&staging, selected_bytes)?;
    assert!(
        fixture_support::replace_selected_font(&selected, &replacement)
            .is_err_and(|error| error.starts_with("create local replacement staging file"))
    );
    assert_eq!(std::fs::read(&selected)?, selected_bytes.as_slice());
    assert_eq!(std::fs::read(&staging)?, selected_bytes.as_slice());
    Ok(())
}

#[test]
fn replacement_rejects_directories_as_font_files() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let source_directory = directory.path().join("source.ttf");
    let selected = directory.path().join("selected.ttf");
    let bytes = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
    std::fs::create_dir(&source_directory)?;
    std::fs::write(&selected, bytes)?;
    assert!(
        fixture_support::replace_selected_font(&selected, &source_directory)
            .is_err_and(|error| error.starts_with("copy local replacement bytes"))
    );
    assert_eq!(std::fs::read(&selected)?, bytes.as_slice());

    let destination_directory = directory.path().join("destination.ttf");
    std::fs::create_dir(&destination_directory)?;
    let sentinel = destination_directory.join("keep.ttf");
    std::fs::write(&sentinel, bytes)?;
    assert!(
        fixture_support::replace_selected_font(&destination_directory, &selected)
            .is_err_and(|error| error.starts_with("replace local selected font"))
    );
    assert_eq!(std::fs::read(&sentinel)?, bytes.as_slice());
    Ok(())
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
