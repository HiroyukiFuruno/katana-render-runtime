use super::super::super::tests::unit_output;
use super::super::capture;
use super::{
    CLASSIC_PROGRAM_COUNT, CaptureState, CaptureSubscriber, SOURCE, observe, poison_capture_lock,
    programs, request, request_json, unit_tempdir, write_capture,
};
use anyhow::{Context, ensure};

#[test]
fn capture_rejects_a_removed_script_after_successful_verification() -> anyhow::Result<()> {
    let tempdir = unit_tempdir()?;
    let capture = write_capture(tempdir.path(), capture_state()?, SOURCE)?;
    capture.verify()?;
    std::fs::remove_file(tempdir.path().join("classic-script-00.js"))?;
    let error = capture
        .verify()
        .err()
        .context("missing script was accepted")?;
    ensure!(
        error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound),
        "unexpected missing script error: {error}"
    );
    Ok(())
}

#[test]
fn existing_manifest_rejects_write_after_capture_programs_are_saved() -> anyhow::Result<()> {
    let tempdir = unit_tempdir()?;
    let root = tempdir.path();
    std::fs::write(root.join("classic-capture.json"), b"reserved")?;
    let error = write_capture(root, capture_state()?, SOURCE)
        .err()
        .context("existing manifest destination accepted")?;
    require_destination_collision(&error)?;
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
fn existing_program_prevents_later_scripts_and_manifest_writes() -> anyhow::Result<()> {
    let artifacts = unit_tempdir()?;
    let root = artifacts.path();
    let reserved = root.join("classic-script-01.js");
    std::fs::write(&reserved, b"reserved")?;
    let error = write_capture(root, capture_state()?, SOURCE)
        .err()
        .context("existing script destination accepted")?;
    require_destination_collision(&error)?;
    ensure!(
        std::fs::read(&reserved)? == b"reserved",
        "existing script overwritten"
    );
    for (name, present) in [
        ("classic-request.json", true),
        ("classic-script-00.js", true),
        ("classic-script-02.js", false),
        ("classic-capture.json", false),
    ] {
        ensure!(
            root.join(name).exists() == present,
            "unexpected state for {name}"
        );
    }
    Ok(())
}

fn require_destination_collision(error: &anyhow::Error) -> anyhow::Result<()> {
    ensure!(
        error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists),
        "unexpected write failure: {error}"
    );
    Ok(())
}

fn capture_state() -> anyhow::Result<CaptureState> {
    let value = request_json(false);
    observe(|| {
        request(&value, CLASSIC_PROGRAM_COUNT);
        programs(&value, false);
    })
}

#[test]
fn capture_without_runtime_events_is_rejected_before_writing() -> anyhow::Result<()> {
    let artifacts = unit_tempdir()?;
    let error = capture(artifacts.path(), SOURCE, || Ok(unit_output()))
        .err()
        .context("capture accepted a render without runtime events")?;
    ensure!(
        error.to_string().contains("classic request not observed"),
        "unexpected capture rejection: {error}"
    );
    ensure!(
        std::fs::read_dir(artifacts.path())?.next().is_none(),
        "capture wrote evidence without runtime events"
    );
    Ok(())
}

#[test]
fn capture_rejects_a_poisoned_runtime_subscriber_before_writing() -> anyhow::Result<()> {
    let artifacts = unit_tempdir()?;
    let error = capture(artifacts.path(), SOURCE, || {
        tracing::dispatcher::get_default(|dispatch| {
            let subscriber = dispatch
                .downcast_ref::<CaptureSubscriber>()
                .context("classic capture subscriber missing")?;
            poison_capture_lock(&subscriber.state)
        })?;
        Ok(unit_output())
    })
    .err()
    .context("capture accepted a poisoned runtime subscriber")?;
    ensure!(
        error.to_string().contains("classic capture lock poisoned"),
        "unexpected capture rejection: {error}"
    );
    ensure!(
        std::fs::read_dir(artifacts.path())?.next().is_none(),
        "partial evidence written"
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
