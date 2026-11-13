use super::*;
use crate::markdown::svg_rasterize::{SvgRasterizeOps, font, text_shaping};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    present: PathBuf,
    absent: PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, String> {
        let root = std::env::temp_dir().join(format!(
            "krr-original-arc-proof-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).map_err(|error| error.to_string())?;
        let fixture = Self {
            present: root.join("present.ttf"),
            absent: root.join("absent.ttf"),
            root,
        };
        std::fs::write(&fixture.present, font::BUNDLED_SANS_SERIF_FONT)
            .map_err(|error| error.to_string())?;
        Ok(fixture)
    }

    fn load(&self) -> Result<(CreationProof, Arc<Database>), String> {
        let mut database = Database::new();
        let mut proof = CreationProof::default();
        for path in [&self.present, &self.absent] {
            proof.load(
                &mut database,
                &[path.to_str().ok_or("fixture path is not UTF-8")?],
            );
        }
        proof.capture_prefix(&database);
        assert!(!database.is_empty());
        assert!(!proof.stamps.is_empty());
        assert_eq!(proof.absent, vec![self.absent.clone()]);
        Ok((proof, Arc::new(database)))
    }

    fn cleanup(&self) -> Result<(), String> {
        std::fs::remove_dir_all(&self.root).map_err(|error| error.to_string())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn transient_publication_rejects_font_changed_after_database_build() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let (proof, database) = fixture.load()?;
    assert!(proof.selected_current(&database));
    let (transient, prefix) = super::super::shared_raster::build(
        &database,
        &proof.prefix,
        &proof.stamps,
        &proof.immutable_shared,
    )
    .ok_or("transient database build failed")?;
    assert!(
        proof
            .finish_transient(&database, transient.clone(), prefix.clone())
            .is_some()
    );

    replace_present_font(&fixture)?;
    assert!(!proof.selected_current(&database));
    assert!(
        proof
            .finish_transient(&database, transient, prefix)
            .is_none()
    );
    fixture.cleanup()?;
    Ok(())
}

fn replace_present_font(fixture: &Fixture) -> Result<(), String> {
    let replacement = fixture.root.join("replacement.ttf");
    let mut changed = font::BUNDLED_SANS_SERIF_FONT.to_vec();
    changed.push(0);
    std::fs::write(&replacement, changed).map_err(|error| error.to_string())?;
    std::fs::rename(replacement, &fixture.present).map_err(|error| error.to_string())
}

#[test]
fn original_arc_prefix_seal_hits_exact_database_without_retaining_owner() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let (proof, database) = fixture.load()?;
    assert!(!proof.original_matches(&database));
    assert!(proof.seal_original(&database));
    assert_original_repeatedly(&proof, &database);
    let mut other = Arc::new(database.as_ref().clone());
    assert_same_local_ids(&other, &database);
    assert!(!proof.original_matches(&other));
    assert!(proof.current(&other));
    mutate_face_weight(&mut other)?;
    assert_eq!(other.len(), database.len());
    assert!(!proof.current(&other));
    assert!(proof.current(&database));
    let weak = Arc::downgrade(&database);
    drop(database);
    assert!(weak.upgrade().is_none());
    assert_seal_released(&proof)?;
    assert!(!proof.original_matches(&other));
    fixture.cleanup()?;
    Ok(())
}

#[test]
fn original_arc_prefix_seal_cow_and_new_database_never_reuse_expired_identity() -> Result<(), String>
{
    let fixture = Fixture::new()?;
    let (proof, mut database) = fixture.load()?;
    assert!(proof.seal_original(&database));
    let old = Arc::downgrade(&database);
    disassociate_original(&proof, &mut database, &old);
    let fresh = Arc::new(database.as_ref().clone());
    assert_same_local_ids(&fresh, &database);
    assert!(!proof.original_matches(&fresh));
    assert!(proof.current(&fresh));
    assert!(proof.seal_original(&fresh));
    /* WHY: expired token の再接続はせず、新 allocation は全 prefix 検証のままにする。 */
    assert!(!proof.original_matches(&fresh));
    drop(database);
    assert!(old.upgrade().is_none());
    assert!(!proof.original_matches(&fresh));
    fixture.cleanup()?;
    Ok(())
}

#[test]
fn original_arc_prefix_seal_present_and_absent_generation_checks_remain_required()
-> Result<(), String> {
    for present in [true, false] {
        check_generation_change(present)?;
    }
    Ok(())
}

fn check_generation_change(present: bool) -> Result<(), String> {
    let fixture = Fixture::new()?;
    let (proof, database) = fixture.load()?;
    assert!(proof.seal_original(&database));
    let attempts = std::cell::Cell::new(0_usize);
    let first_completed = std::cell::Cell::new(false);
    SvgRasterizeOps::with_html_font_render_scope(|| -> Result<(), String> {
        attempts.set(attempts.get() + 1);
        if attempts.get() > 1 {
            assert_eq!(attempts.get(), 2);
            assert!(!text_shaping::known_memo_usable());
            assert!(first_completed.get());
            return Ok(());
        }
        check_first_generation_attempt(&fixture, &proof, &database, present)?;
        first_completed.set(true);
        Ok(())
    })?;
    assert!(first_completed.get());
    assert_eq!(attempts.get(), if present { 2 } else { 1 });
    assert!(!proof.current(&database));
    assert!(!proof.seal_original(&database));
    fixture.cleanup()?;
    Ok(())
}

#[test]
fn original_arc_prefix_seal_load_and_recapture_invalidate_binding() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let (mut proof, mut database) = fixture.load()?;
    assert!(proof.seal_original(&database));
    proof.capture_prefix(&database);
    assert!(!proof.original_matches(&database));
    assert!(proof.current(&database));
    assert!(proof.seal_original(&database));
    let another_absent = fixture.root.join("also-absent.ttf");
    proof.load(
        Arc::make_mut(&mut database),
        &[another_absent.to_str().ok_or("fixture path is not UTF-8")?],
    );
    assert!(!proof.original_matches(&database));
    assert!(proof.current(&database));
    assert!(proof.seal_original(&database));
    assert!(proof.original_matches(&database));
    fixture.cleanup()?;
    Ok(())
}

fn assert_same_local_ids(first: &Database, second: &Database) {
    assert_eq!(first.len(), second.len());
    assert_eq!(
        first.faces().map(|face| face.id).collect::<Vec<_>>(),
        second.faces().map(|face| face.id).collect::<Vec<_>>()
    );
}

fn assert_seal_released(proof: &CreationProof) -> Result<(), String> {
    assert!(
        proof
            .original_database
            .get()
            .ok_or("seal missing")?
            .upgrade()
            .is_none()
    );
    Ok(())
}

fn mutate_face_weight(other: &mut Arc<Database>) -> Result<(), String> {
    let id = other.faces().next().ok_or("real face missing")?.id;
    let mut face = other.face(id).ok_or("real face missing")?.clone();
    face.weight = resvg::usvg::fontdb::Weight(900);
    Arc::make_mut(other).remove_face(id);
    Arc::make_mut(other).push_face_info(face);
    Ok(())
}

fn check_first_generation_attempt(
    fixture: &Fixture,
    proof: &CreationProof,
    database: &Arc<Database>,
    present: bool,
) -> Result<(), String> {
    assert!(text_shaping::known_memo_usable());
    assert!(proof.original_matches(database));
    assert!(proof.current(database));
    assert!(proof.selected_current(database));
    if present {
        let replacement = fixture.root.join("replacement.ttf");
        std::fs::write(&replacement, font::BUNDLED_SANS_SERIF_FONT)
            .map_err(|error| error.to_string())?;
        std::fs::rename(replacement, &fixture.present).map_err(|error| error.to_string())?;
    } else {
        std::fs::write(&fixture.absent, font::BUNDLED_SANS_SERIF_FONT)
            .map_err(|error| error.to_string())?;
        assert!(!proof.current(database));
    }
    assert!(proof.original_matches(database));
    assert!(!proof.selected_current(database));
    Ok(())
}

fn assert_original_repeatedly(proof: &CreationProof, database: &Arc<Database>) {
    let strong = Arc::strong_count(database);
    for _ in 0..3 {
        assert!(proof.original_matches(database));
        assert!(proof.current(database));
        assert_eq!(Arc::strong_count(database), strong);
    }
}

fn disassociate_original(
    proof: &CreationProof,
    database: &mut Arc<Database>,
    old: &std::sync::Weak<Database>,
) {
    assert!(Arc::get_mut(database).is_none());
    let _ = Arc::make_mut(database);
    assert!(old.upgrade().is_none());
    assert!(!proof.original_matches(database));
    assert!(proof.current(database));
}

#[path = "svg_rasterize_html_font_creation_boundary_tests.rs"]
mod boundary_tests;
