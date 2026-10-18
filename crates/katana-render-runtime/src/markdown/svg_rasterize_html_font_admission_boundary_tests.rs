use super::super::finish_admission;
use super::{
    Arc, CreationProof, Database, OwnedFontCopy, REGULAR_TEXT, SvgRasterizeOps, admitted,
    regular_admitted,
};

const REGULAR_FONT_WEIGHT: u16 = 400;

fn with_absent_path(
    copied: &OwnedFontCopy,
) -> Result<(Arc<Database>, CreationProof, std::path::PathBuf), String> {
    let absent = copied.directory.join("later.ttf");
    let mut database = Database::new();
    let mut proof = CreationProof::default();
    proof.load(
        &mut database,
        &[
            copied.path.to_str().ok_or("nonUTF8 font path")?,
            absent.to_str().ok_or("nonUTF8 absent path")?,
        ],
    );
    proof.capture_prefix(&database);
    Ok((Arc::new(database), proof, absent))
}

#[test]
fn absent_font_appearance_between_admission_entry_and_exit_rejects_known_result()
-> Result<(), String> {
    check_absent_appearance(false)
}

#[test]
fn absent_directory_appearance_between_admission_entry_and_exit_rejects_known_result()
-> Result<(), String> {
    check_absent_appearance(true)
}

fn check_absent_appearance(directory: bool) -> Result<(), String> {
    let copied = OwnedFontCopy::from_real_arial()?;
    let (database, proof, absent) = with_absent_path(&copied)?;
    let selected = admitted(
        &database,
        &proof,
        "Arial",
        REGULAR_TEXT,
        REGULAR_FONT_WEIGHT,
    )
    .ok_or("healthy known admission failed")?;
    assert_appearance_during_scope(&database, &proof, selected, &copied, &absent, directory)?;
    assert!(regular_admitted(&database, &proof).is_none());
    if directory {
        std::fs::remove_dir(&absent).map_err(|error| error.to_string())?;
    }
    let (fresh_database, fresh_proof, _) = with_absent_path(&copied)?;
    if directory {
        assert!(regular_admitted(&fresh_database, &fresh_proof).is_some());
    } else {
        assert!(!fresh_proof.current(&database));
        assert!(regular_admitted(&fresh_database, &fresh_proof).is_some());
    }
    Ok(())
}

fn assert_appearance_during_scope(
    database: &Arc<Database>,
    proof: &CreationProof,
    selected: resvg::usvg::fontdb::ID,
    copied: &OwnedFontCopy,
    absent: &std::path::Path,
    directory: bool,
) -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| -> Result<(), String> {
        assert!(proof.current(database));
        assert_eq!(
            finish_admission(database, proof, selected, REGULAR_TEXT),
            Some(selected)
        );
        if directory {
            std::fs::create_dir(absent).map_err(|error| error.to_string())?;
        } else {
            std::fs::copy(&copied.path, absent).map_err(|error| error.to_string())?;
        }
        assert!(finish_admission(database, proof, selected, REGULAR_TEXT).is_none());
        Ok(())
    })
}

#[test]
fn atomic_file_replacement_at_admission_exit_retries_without_old_proof() -> Result<(), String> {
    let (database, proof, copied) = super::copied_arial_database()?;
    let selected = admitted(
        &database,
        &proof,
        "Arial",
        REGULAR_TEXT,
        REGULAR_FONT_WEIGHT,
    )
    .ok_or("healthy known admission failed")?;
    let mut attempts = 0;
    let result = SvgRasterizeOps::with_html_font_render_scope(|| -> Result<_, String> {
        attempts += 1;
        if attempts == 1 {
            assert!(proof.current(&database));
            let replacement = copied.directory.join("replacement.ttf");
            std::fs::copy(&copied.path, &replacement).map_err(|error| error.to_string())?;
            std::fs::rename(replacement, &copied.path).map_err(|error| error.to_string())?;
        }
        Ok(finish_admission(&database, &proof, selected, REGULAR_TEXT))
    })?;
    assert_eq!(attempts, 2);
    assert!(result.is_none());
    assert!(!proof.current(&database));
    assert_fresh_admission(&copied)
}

fn assert_fresh_admission(copied: &OwnedFontCopy) -> Result<(), String> {
    let mut fresh_database = Database::new();
    let mut fresh_proof = CreationProof::default();
    fresh_proof.load(
        &mut fresh_database,
        &[copied.path.to_str().ok_or("nonUTF8 font path")?],
    );
    fresh_proof.capture_prefix(&fresh_database);
    assert!(
        admitted(
            &Arc::new(fresh_database),
            &fresh_proof,
            "Arial",
            REGULAR_TEXT,
            400
        )
        .is_some()
    );
    Ok(())
}
