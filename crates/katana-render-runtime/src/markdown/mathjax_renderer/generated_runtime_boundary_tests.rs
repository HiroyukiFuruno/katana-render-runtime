use super::MathJaxGeneratedRuntimeAsset;

#[test]
fn generated_runtime_reports_write_failure_for_a_dangling_target()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let missing = directory.path().join("absent").join("bundle.js");
    let path = directory.path().join("bundle.js");
    std::os::unix::fs::symlink(&missing, &path)?;
    let expected = std::fs::read(&path).err().ok_or("target was readable")?;
    assert_eq!(expected.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        MathJaxGeneratedRuntimeAsset::materialize_at(path),
        Err(expected.to_string())
    );
    assert!(!missing.exists());
    Ok(())
}
