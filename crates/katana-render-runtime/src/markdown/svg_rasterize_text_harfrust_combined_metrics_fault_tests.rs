use super::super::{FontShape, combined_metrics_probe::combined_run_with_hook};
use super::{FontFaceCacheScope, commit_file_font, font_for_face};
use resvg::usvg::fontdb::{Database, ID, Source};
use std::{
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;
const BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    path: PathBuf,
    database: Database,
    id: ID,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let path = std::env::temp_dir().join(format!(
            "krr-combined-fault-{}-{}.ttf",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        file.write_all(BYTES)?;
        let mut database = Database::new();
        database.load_font_source(Source::File(path.clone()));
        let id = database.faces().next().ok_or("file face missing")?.id;
        Ok(Self { path, database, id })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn poison_during_measurement(fixture: &Fixture) -> TestResult<()> {
    let mut reached_commit = false;
    let mut poisoned_active_handle = false;
    let result = combined_run_with_hook(
        &fixture.database,
        fixture.id,
        "Animal",
        FontShape::new(18.25, 700, false),
        None,
        |active| {
            reached_commit = true;
            let Some(poisoned) = active.poisoned.as_ref() else {
                return;
            };
            /* WHY: 実際に計測したhandleを毒化し、commit時の公開抑止を観測する。 */
            poisoned.store(true, Ordering::Release);
            poisoned_active_handle = poisoned.load(Ordering::Acquire);
        },
    );
    assert!(reached_commit);
    assert!(
        poisoned_active_handle,
        "poison flag missing for the measured handle"
    );
    assert!(result.is_none());
    assert!(super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    Ok(())
}

#[test]
fn generation_change_after_both_measurements_publishes_no_combined_result() -> TestResult<()> {
    let fixture = Fixture::new()?;
    let _scope = FontFaceCacheScope::enter();
    let shape = FontShape::new(18.25, 700, false);
    let mut mutation = Ok(());
    let mut reached_commit = false;
    let result =
        combined_run_with_hook(&fixture.database, fixture.id, "Animal", shape, None, |_| {
            reached_commit = true;
            /* WHY: 完了した幅とセルを、異なる世代へ誤って公開する経路を再現する。 */
            let mut changed = BYTES.to_vec();
            changed.push(0);
            mutation = std::fs::write(&fixture.path, changed);
        });
    mutation?;
    assert!(
        reached_commit,
        "both measurements did not finish before mutation"
    );
    assert!(result.is_none());
    assert!(super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    assert!(
        combined_run_with_hook(&fixture.database, fixture.id, "Animal", shape, None, |_| {})
            .is_some()
    );
    Ok(())
}

#[test]
fn poisoned_same_file_handle_publishes_no_width_or_cell_and_reloads() -> TestResult<()> {
    let fixture = Fixture::new()?;
    let _scope = FontFaceCacheScope::enter();
    poison_during_measurement(&fixture)?;
    let fresh = font_for_face(&fixture.database, fixture.id).ok_or("fresh handle missing")?;
    assert!(
        !fresh
            .poisoned
            .as_ref()
            .ok_or("fresh poison flag missing")?
            .load(Ordering::Acquire)
    );
    assert!(commit_file_font(&fixture.database, fixture.id, &fresh));
    Ok(())
}
