use super::{CaptureState, CaptureSubscriber, TARGET, write_capture};
use anyhow::{Context, ensure};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[path = "render_attestation_classic_typed_fields_tests.rs"]
mod typed_fields;

const SOURCE: &str = "flowchart TD\n  A[日本語] --> B";
static SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn request_json(zenuml: bool) -> String {
    json!({"source":SOURCE,"svgId":"protocol-only","theme":"default",
        "background":"white","fill":"white","text":"black","stroke":"black",
        "arrow":"black","diagramType":if zenuml {"zenuml"} else {"flowchart"}})
    .to_string()
}

fn request(value: &str, count: u64) {
    tracing::event!(target: TARGET, tracing::Level::TRACE,
        phase = "request", request_json = value, script_count = count);
}

fn script(name: &str, code: &str) {
    tracing::event!(target: TARGET, tracing::Level::TRACE,
        phase = "script", name = name, code = code);
}

fn complete() {
    tracing::event!(target: TARGET, tracing::Level::TRACE, phase = "complete");
}

fn programs(value: &str, zenuml: bool) {
    for name in [
        "mermaid-runtime.min.js",
        "mermaid.min.js",
        "mermaid-zenuml.min.js",
    ] {
        script(name, "/* protocol-only bytes: 日本語, \"quoted\"\n */");
        complete();
    }
    if zenuml {
        script("zenuml-runtime.min.js", "/* protocol-only ZenUML bytes */");
        complete();
    }
    script(
        "render-mermaid.js",
        &format!("katanaRunMermaidRuntime({value});"),
    );
    complete();
}

fn observe(events: impl FnOnce()) -> anyhow::Result<CaptureState> {
    let state = Arc::new(Mutex::new(CaptureState::new()));
    let subscriber = CaptureSubscriber {
        state: Arc::clone(&state),
    };
    tracing::subscriber::with_default(subscriber, events);
    Arc::try_unwrap(state)
        .map_err(|_| anyhow::anyhow!("protocol subscriber escaped"))?
        .into_inner()
        .map_err(|_| anyhow::anyhow!("protocol lock poisoned"))
}

fn rejected(state: CaptureState, reason: &str) -> anyhow::Result<()> {
    let error = state
        .validate(SOURCE)
        .err()
        .context("protocol unexpectedly accepted")?;
    ensure!(
        error.to_string().contains(reason),
        "wrong rejection: {error}"
    );
    Ok(())
}

#[test]
fn normal_and_zenuml_protocols_preserve_raw_strings() -> anyhow::Result<()> {
    for zenuml in [false, true] {
        let value = request_json(zenuml);
        let state = observe(|| {
            request(&value, if zenuml { 5 } else { 4 });
            programs(&value, zenuml);
        })?;
        state.validate(SOURCE)?;
        ensure!(
            state.request.as_deref() == Some(value.as_str()),
            "request bytes differ"
        );
        ensure!(
            state.scripts[0].code.contains("日本語, \"quoted\"\n"),
            "code escaped"
        );
    }
    Ok(())
}

#[test]
fn missing_cache_hit_and_incomplete_evaluation_are_rejected() -> anyhow::Result<()> {
    rejected(observe(|| {})?, "not observed")?;
    let value = request_json(false);
    rejected(observe(|| request(&value, 4))?, "script missing")?;
    rejected(
        observe(|| {
            request(&value, 4);
            script("mermaid-runtime.min.js", "protocol-only");
            script("mermaid.min.js", "protocol-only");
        })?,
        "script incomplete",
    )
}

#[test]
fn wrong_order_duplicate_and_extra_events_are_rejected() -> anyhow::Result<()> {
    let value = request_json(false);
    rejected(
        observe(|| {
            request(&value, 4);
            script("mermaid.min.js", "protocol-only");
        })?,
        "order differs",
    )?;
    rejected(
        observe(|| {
            request(&value, 4);
            request(&value, 4);
        })?,
        "duplicate classic request",
    )?;
    rejected(
        observe(|| {
            request(&value, 4);
            programs(&value, false);
            script("extra.js", "protocol-only");
        })?,
        "extra classic script",
    )
}

#[test]
fn completion_without_script_and_double_completion_are_rejected() -> anyhow::Result<()> {
    rejected(observe(complete)?, "has no script")?;
    let value = request_json(false);
    rejected(
        observe(|| {
            request(&value, 4);
            script("mermaid-runtime.min.js", "protocol-only");
            complete();
            complete();
        })?,
        "duplicate classic completion",
    )
}

#[test]
fn request_source_schema_and_program_bytes_are_bound() -> anyhow::Result<()> {
    let value = request_json(false);
    let state = observe(|| {
        request(&value, 4);
        programs(&value, false);
    })?;
    ensure!(
        state.validate("different input").is_err(),
        "source substitution accepted"
    );
    let mut state = observe(|| {
        request(&value, 4);
        programs(&value, false);
    })?;
    state.request = Some("{}".into());
    rejected(state, "field count differs")?;
    let mut state = observe(|| {
        request(&value, 4);
        programs(&value, false);
    })?;
    state
        .scripts
        .last_mut()
        .context("last script missing")?
        .code
        .push(' ');
    rejected(state, "request program differs")
}

#[test]
fn foreign_thread_events_are_rejected() -> anyhow::Result<()> {
    let state = observe(|| {
        tracing::dispatcher::get_default(|dispatch| {
            let dispatch = dispatch.clone();
            let thread = std::thread::spawn(move || {
                tracing::dispatcher::with_default(&dispatch, complete);
            });
            assert!(thread.join().is_ok());
        });
    })?;
    rejected(state, "thread differs")
}

#[test]
fn artifact_bytes_are_verified_and_existing_destinations_rejected() -> anyhow::Result<()> {
    let value = request_json(false);
    let state = observe(|| {
        request(&value, 4);
        programs(&value, false);
    })?;
    state.validate(SOURCE)?;
    verify_artifact_bytes_and_reject_existing_destination(&value, state)
}

fn verify_artifact_bytes_and_reject_existing_destination(
    value: &str,
    state: CaptureState,
) -> anyhow::Result<()> {
    let parent = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../tmp/quality/mermaid-attestation-classic-capture-implementation-r1/unit-artifacts",
    );
    std::fs::create_dir_all(&parent)?;
    let root = std::fs::canonicalize(parent)?.join(format!(
        "{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root)?;
    let capture = write_capture(&root, state)?;
    capture.verify()?;
    ensure!(
        std::fs::read_to_string(root.join("classic-request.json"))? == value,
        "request changed"
    );
    reject_changed_or_overwritten_artifacts(&root, value, || capture.verify())
}

fn reject_changed_or_overwritten_artifacts(
    root: &std::path::Path,
    value: &str,
    verify_capture: impl Fn() -> anyhow::Result<serde_json::Value>,
) -> anyhow::Result<()> {
    std::fs::write(root.join("classic-script-00.js"), "changed")?;
    ensure!(verify_capture().is_err(), "changed artifact accepted");
    let state = observe(|| {
        request(value, 4);
        programs(value, false);
    })?;
    ensure!(
        write_capture(root, state).is_err(),
        "existing artifact overwritten"
    );
    Ok(())
}
