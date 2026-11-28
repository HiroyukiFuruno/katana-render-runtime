use super::{Fixture, Inspector, repository_root};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEST_NAMESPACE: AtomicUsize = AtomicUsize::new(0);

struct FixturePaths {
    repository: PathBuf,
    namespace: PathBuf,
    fixtures: PathBuf,
    success: PathBuf,
    failure: PathBuf,
    occupied: PathBuf,
    adjacent: PathBuf,
    missing: String,
}

impl FixturePaths {
    fn create() -> Result<Self, Box<dyn std::error::Error>> {
        let repository = repository_root()?;
        let suffix = NEXT_TEST_NAMESPACE.fetch_add(1, Ordering::Relaxed);
        let namespace = repository.join("tmp/quality").join(format!(
            "mathjax-copy-inputs-cleanup-{}-{suffix}",
            std::process::id()
        ));
        let fixtures = namespace.join("fixtures");
        let missing = format!(
            "tmp/quality/missing-copy-input-{}-{suffix}",
            std::process::id()
        );
        if repository.join(&missing).exists() {
            return Err("missing test input unexpectedly exists".into());
        }
        fs::create_dir_all(namespace.parent().ok_or("test namespace has no parent")?)?;
        fs::create_dir(&namespace)?;
        let paths = Self {
            repository,
            namespace,
            success: fixtures.join("success"),
            failure: fixtures.join("failure"),
            occupied: fixtures.join("occupied"),
            adjacent: fixtures.join("adjacent"),
            fixtures,
            missing,
        };
        fs::create_dir(&paths.fixtures)?;
        Ok(paths)
    }
}

impl Drop for FixturePaths {
    fn drop(&mut self) {
        /* WHY: create_dirで排他的に確保したnamespaceだけを回収する。 */
        if self.namespace.exists() {
            let _ = fs::remove_dir_all(&self.namespace);
        }
    }
}

#[test]
fn copy_inputs_copies_the_requested_file_and_drop_removes_its_root()
-> Result<(), Box<dyn std::error::Error>> {
    let paths = FixturePaths::create()?;
    let fixture = Fixture::copy_inputs_at(
        &paths.repository,
        paths.success.clone(),
        &["Cargo.toml"],
        Inspector::MathJax,
    )?;
    assert!(fixture.root().join("Cargo.toml").is_file());
    drop(fixture);
    assert!(!paths.success.exists());
    Ok(())
}

#[test]
fn copy_inputs_removes_its_root_after_a_later_copy_fails() -> Result<(), Box<dyn std::error::Error>>
{
    let paths = FixturePaths::create()?;
    let result = Fixture::copy_inputs_at(
        &paths.repository,
        paths.failure.clone(),
        &["Cargo.toml", paths.missing.as_str()],
        Inspector::MathJax,
    );
    assert!(result.is_err(), "copying the absent second input must fail");
    assert!(
        !paths.failure.exists(),
        "failed fixture root must be removed"
    );
    Ok(())
}

#[test]
fn copy_inputs_rejects_an_existing_root_without_changing_it_or_its_neighbor()
-> Result<(), Box<dyn std::error::Error>> {
    let paths = FixturePaths::create()?;
    fs::create_dir_all(&paths.occupied)?;
    fs::write(paths.occupied.join("sentinel"), b"occupied-root")?;
    fs::create_dir_all(&paths.adjacent)?;
    fs::write(paths.adjacent.join("sentinel"), b"adjacent-fixture")?;

    let result = Fixture::copy_inputs_at(
        &paths.repository,
        paths.occupied.clone(),
        &["Cargo.toml", paths.missing.as_str()],
        Inspector::MathJax,
    );
    assert!(result.is_err(), "an existing root must not be claimed");
    assert_eq!(fs::read(paths.occupied.join("sentinel"))?, b"occupied-root");
    assert!(!paths.occupied.join("Cargo.toml").exists());
    assert_eq!(
        fs::read(paths.adjacent.join("sentinel"))?,
        b"adjacent-fixture"
    );
    Ok(())
}
