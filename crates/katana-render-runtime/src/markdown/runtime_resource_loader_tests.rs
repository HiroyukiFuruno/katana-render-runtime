use super::{BundledResource, RuntimeResourceLoader};
use std::borrow::Cow;

const CONCURRENT_READERS: usize = 4;
const INVALID_UTF8: [u8; 1] = [0xff];

#[test]
fn bundled_resources_share_the_same_immutable_source() -> std::io::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("missing");
    for kind in [
        BundledResource::Mermaid,
        BundledResource::Drawio,
        BundledResource::MathJax,
    ] {
        let first = RuntimeResourceLoader::read(&path, Some(kind))?;
        let second = RuntimeResourceLoader::read(&path, Some(kind))?;
        assert!(matches!(&first, Cow::Borrowed(_)));
        assert_eq!(first.as_ptr(), second.as_ptr());
        assert_eq!(first, second);
        let loaded = RuntimeResourceLoader::load(&path, Some(kind))?;
        assert_eq!(loaded.text().as_ptr(), first.as_ptr());
        assert!(loaded.digest().is_none());
        assert!(
            RuntimeResourceLoader::read(&path, None)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        );
    }
    Ok(())
}

#[test]
fn bundled_script_text_rejects_invalid_utf8() {
    assert!(
        RuntimeResourceLoader::bundled_script_text(&INVALID_UTF8)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::InvalidData)
    );
}

#[test]
fn external_resources_reflect_replacement_removal_and_invalid_utf8() -> std::io::Result<()> {
    let dir = tempfile::tempdir()?;
    for extension in ["js", "svg", "xml"] {
        let path = dir.path().join(format!("resource.{extension}"));
        check_updates(&path, &dir.path().join("replacement"))?;
        check_read_failures(&path)?;
    }
    Ok(())
}

fn check_updates(path: &std::path::Path, replacement: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(path, "first")?;
    assert!(matches!(
        RuntimeResourceLoader::read(path, None)?,
        Cow::Owned(text) if text == "first"
    ));
    let first = RuntimeResourceLoader::load(path, None)?;
    let repeated = RuntimeResourceLoader::load(path, None)?;
    assert_eq!(first.digest(), repeated.digest());
    std::fs::write(path, "second")?;
    assert!(matches!(
        RuntimeResourceLoader::read_file(path).as_deref(),
        Ok("second")
    ));
    let changed = RuntimeResourceLoader::load(path, None)?;
    assert_ne!(first.digest(), changed.digest());
    std::fs::write(replacement, "replacement")?;
    #[cfg(windows)]
    std::fs::remove_file(path)?;
    std::fs::rename(replacement, path)?;
    assert!(matches!(
        RuntimeResourceLoader::read_file(path).as_deref(),
        Ok("replacement")
    ));
    check_parallel_reads(path);
    assert_eq!(first.text(), "first");
    assert_eq!(changed.text(), "second");
    Ok(())
}

fn check_parallel_reads(path: &std::path::Path) {
    std::thread::scope(|scope| {
        for _ in 0..CONCURRENT_READERS {
            scope.spawn(move || {
                assert!(matches!(
                    RuntimeResourceLoader::read_file(path).as_deref(),
                    Ok("replacement")
                ));
            });
        }
    });
}

fn check_read_failures(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(path, INVALID_UTF8)?;
    assert_external_load_error(path, std::io::ErrorKind::InvalidData);
    std::fs::remove_file(path)?;
    assert_external_load_error(path, std::io::ErrorKind::NotFound);
    Ok(())
}

fn assert_external_load_error(path: &std::path::Path, kind: std::io::ErrorKind) {
    assert!(RuntimeResourceLoader::read_file(path).is_err_and(|error| error.kind() == kind));
    assert!(RuntimeResourceLoader::load(path, None).is_err_and(|error| error.kind() == kind));
}
