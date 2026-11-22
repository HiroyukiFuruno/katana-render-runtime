use super::super::test_support::physical_font_fixture;
use super::{physical_file, resolve_observer_face};
use crate::markdown::chrome_font_identity_probe_tests::ObserverFace;

#[test]
fn retained_observer_face_requires_the_selected_physical_font() -> Result<(), String> {
    let (temp, _, selected) = physical_font_fixture()?;
    let (file_path, file_sha256, _) = physical_file(&temp.path().join("physical-font.ttf"))?;
    let face = ObserverFace {
        post_script_name: selected.post_script_name.clone(),
        file_path: file_path.clone(),
        file_sha256,
        face_index: 0,
    };
    let resolved = resolve_observer_face(&selected, &[face])?;
    assert_eq!(resolved.identity.path, file_path);
    assert_eq!(resolved.identity.face_index, 0);
    assert_eq!(resolved.method, "Chrome retained font observation");
    Ok(())
}

#[test]
fn retained_observer_face_rejects_changed_or_wrong_face_proofs() -> Result<(), String> {
    let (temp, _, selected) = physical_font_fixture()?;
    let (file_path, file_sha256, _) = physical_file(&temp.path().join("physical-font.ttf"))?;
    for (file_sha256, face_index) in [("0".repeat(64), 0), (file_sha256, 1)] {
        let face = ObserverFace {
            post_script_name: selected.post_script_name.clone(),
            file_path: file_path.clone(),
            file_sha256,
            face_index,
        };
        assert!(resolve_observer_face(&selected, &[face]).is_err());
    }
    Ok(())
}

#[test]
fn retained_observer_face_requires_one_matching_proof() -> Result<(), String> {
    let (temp, _, selected) = physical_font_fixture()?;
    let (file_path, file_sha256, _) = physical_file(&temp.path().join("physical-font.ttf"))?;
    let face = ObserverFace {
        post_script_name: selected.post_script_name.clone(),
        file_path,
        file_sha256,
        face_index: 0,
    };
    let missing = ObserverFace {
        post_script_name: "KRR-unknown-observer-face".to_string(),
        ..face.clone()
    };
    assert!(resolve_observer_face(&selected, &[missing]).is_err());
    assert!(resolve_observer_face(&selected, &[face.clone(), face]).is_err());
    Ok(())
}
