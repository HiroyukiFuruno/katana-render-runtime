use super::super::super::tests::unit_output;
use super::super::capture;
use super::{
    CLASSIC_PROGRAM_COUNT, SOURCE, observe, programs, request, request_json, unit_tempdir,
    write_capture,
};
use anyhow::{Context, ensure};

#[test]
fn existing_manifest_rejects_write_after_capture_programs_are_saved() -> anyhow::Result<()> {
    let value = request_json(false);
    let state = observe(|| {
        request(&value, CLASSIC_PROGRAM_COUNT);
        programs(&value, false);
    })?;
    state.validate(SOURCE)?;
    let tempdir = unit_tempdir()?;
    let root = tempdir.path();
    std::fs::write(root.join("classic-capture.json"), b"reserved")?;
    let error = write_capture(root, state)
        .err()
        .context("existing manifest destination accepted")?;
    ensure!(
        error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists),
        "manifest write failed for an unexpected reason: {error}"
    );
    ensure!(
        root.join("classic-script-03.js").exists(),
        "capture stopped before manifest write"
    );
    ensure!(
        std::fs::read(root.join("classic-capture.json"))? == b"reserved",
        "existing manifest was overwritten"
    );
    Ok(())
}

#[test]
fn capture_rejects_a_subscriber_retained_after_render() -> anyhow::Result<()> {
    let artifacts = unit_tempdir()?;
    let mut escaped_dispatch = None;
    let error = capture(artifacts.path(), SOURCE, || {
        tracing::dispatcher::get_default(|dispatch| escaped_dispatch = Some(dispatch.clone()));
        Ok(unit_output())
    })
    .err()
    .context("capture accepted a subscriber retained after render")?;
    ensure!(
        error
            .to_string()
            .contains("classic subscriber escaped render"),
        "unexpected capture rejection: {error}"
    );
    ensure!(
        std::fs::read_dir(artifacts.path())?.next().is_none(),
        "capture wrote evidence before rejecting the escaped subscriber"
    );
    drop(escaped_dispatch);
    Ok(())
}
