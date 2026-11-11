use super::{BundledResource, RuntimeResourceLoader};

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
        RuntimeResourceLoader::read_file(path).as_deref(),
        Ok("first")
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
    assert!(
        RuntimeResourceLoader::read_file(path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::InvalidData)
    );
    std::fs::remove_file(path)?;
    assert!(
        RuntimeResourceLoader::read_file(path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    );
    Ok(())
}
