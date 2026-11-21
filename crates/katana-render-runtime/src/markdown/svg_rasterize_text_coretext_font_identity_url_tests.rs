use super::super::file_path_from_url_attribute;
use objc2_core_foundation::{CFString, CFURL};

#[test]
fn coretext_url_identity_rejects_non_url_attributes_and_unrepresentable_paths() -> Result<(), String>
{
    let path = std::path::PathBuf::from("/font.ttf");
    let url = CFURL::from_file_path(&path).ok_or("valid CoreFoundation URL missing")?;
    assert_eq!(file_path_from_url_attribute(Some(url.into())), Some(path));

    let wrong_type = CFString::from_str("/font.ttf");
    assert!(file_path_from_url_attribute(Some(wrong_type.into())).is_none());

    let long_path = format!("/{}", "font/".repeat(512));
    let url = CFURL::from_string(
        None,
        &CFString::from_str(&format!("file://{long_path}")),
        None,
    )
    .ok_or("CoreFoundation rejected the long URL")?;
    assert!(file_path_from_url_attribute(Some(url.into())).is_none());
    Ok(())
}
