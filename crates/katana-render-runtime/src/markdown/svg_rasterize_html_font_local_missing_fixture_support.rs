use resvg::usvg::fontdb::{Database, Source};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{ACTIVE, Fixture, Mutation, Scope};

fn local_io<T>(context: &str, result: std::io::Result<T>) -> Result<T, String> {
    result.map_err(|error| format!("{context}: {error}"))
}

pub(super) fn require_absent(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "local missing-font path already exists: {}",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn canonical_parent(path: &Path) -> Result<PathBuf, String> {
    path.parent()
        .ok_or_else(|| "local fixture has no parent directory".to_owned())?
        .canonicalize()
        .map_err(|error| format!("canonicalize local fixture root: {error}"))
}

pub(super) fn require_within_root(path: &Path, root: &Path, label: &str) -> Result<(), String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("canonicalize local {label}: {error}"))?;
    if !canonical.starts_with(root) {
        return Err(format!("local {label} escaped fixture root"));
    }
    Ok(())
}

pub(super) fn validate_database_sources(database: &Database, root: &Path) -> Result<(), String> {
    for face in database.faces() {
        match &face.source {
            Source::File(path) => require_within_root(path, root, "font source")?,
            Source::Binary(_) => {}
            Source::SharedFile(_, _) => {
                return Err("shared font source is not fixture-local".into());
            }
        }
    }
    Ok(())
}

pub(super) fn validate_fixture(database: &Database, mutation: &Mutation) -> Result<(), String> {
    let path = match mutation {
        Mutation::Missing(path) => path,
        Mutation::Replace { selected, .. } => selected,
    };
    let root = canonical_parent(path)?;
    validate_database_sources(database, &root)?;
    if let Mutation::Replace {
        selected,
        replacement,
    } = mutation
    {
        require_within_root(selected, &root, "selected font")?;
        require_within_root(replacement, &root, "replacement font")?;
    }
    Ok(())
}

pub(super) fn create_missing_font(path: &Path) -> Result<(), String> {
    use std::io::Write;

    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path);
    let mut file = local_io("create local missing font", file)?;
    local_io(
        "write local missing font bytes",
        file.write_all(crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT),
    )
}

pub(super) fn replace_selected_font(selected: &Path, replacement: &Path) -> Result<(), String> {
    use std::io::Write;

    let staged = selected.with_extension("admission-replacement");
    let mut input = local_io(
        "open copied public replacement font",
        std::fs::File::open(replacement),
    )?;
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged);
    let mut output = local_io("create local replacement staging file", output)?;
    local_io(
        "copy local replacement bytes",
        std::io::copy(&mut input, &mut output),
    )?;
    local_io("flush local replacement bytes", output.flush())?;
    drop(output);
    local_io(
        "replace local selected font",
        std::fs::rename(staged, selected),
    )
}

impl Scope {
    pub(in super::super::super) fn mutated(&self) -> bool {
        ACTIVE.with(|active| {
            active
                .borrow()
                .as_ref()
                .is_some_and(|fixture| fixture.mutated)
        })
    }

    pub(in super::super::super) fn mutation_result(&self) -> Result<(), String> {
        ACTIVE.with(|active| {
            let active = active.borrow();
            let fixture = active.as_ref().ok_or("local fixture scope missing")?;
            fixture
                .mutation_error
                .as_ref()
                .map_or(Ok(()), |error| Err(error.clone()))
        })
    }

    pub(in super::super::super) fn mappings_released(&self) -> bool {
        ACTIVE.with(|active| {
            active.borrow().as_ref().is_some_and(|fixture| {
                !fixture.mapped.is_empty()
                    && fixture.mapped.iter().all(|data| data.upgrade().is_none())
            })
        })
    }
}

pub(super) fn after_admission() {
    ACTIVE.with(|active| {
        if let Some(fixture) = active.borrow_mut().as_mut().filter(|fixture| {
            fixture.before_acquisition && !fixture.mutated && fixture.mutation_error.is_none()
        }) {
            record_mutation(fixture);
        }
    });
}

pub(super) fn after_selector(database: &Database, admitted: bool) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(fixture) = active.as_mut() else {
            return;
        };
        if admitted
            && !fixture.before_acquisition
            && !fixture.mutated
            && fixture.mutation_error.is_none()
        {
            record_mutation(fixture);
        }
        if fixture.mutated && fixture.mapped.is_empty() {
            fixture.mapped = database
                .faces()
                .filter_map(|face| match &face.source {
                    Source::SharedFile(_, data) => Some(Arc::downgrade(data)),
                    _ => None,
                })
                .collect();
        }
    });
}

fn record_mutation(fixture: &mut Fixture) {
    let result = match &fixture.mutation {
        Mutation::Missing(path) => create_missing_font(path),
        Mutation::Replace {
            selected,
            replacement,
        } => replace_selected_font(selected, replacement),
    };
    if let Err(error) = result {
        fixture.mutation_error = Some(error);
    } else {
        fixture.mutated = true;
    }
}
