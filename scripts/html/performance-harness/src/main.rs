#[cfg(all(feature = "baseline", feature = "candidate"))]
compile_error!("select exactly one measurement arm");
#[cfg(not(any(feature = "baseline", feature = "candidate")))]
compile_error!("select the baseline or candidate measurement arm");

#[cfg(feature = "baseline")]
use baseline::HtmlBrowserViewport;
#[cfg(feature = "candidate")]
use candidate::HtmlBrowserViewport;
#[cfg(feature = "candidate")]
extern crate candidate as katana_render_runtime;
use std::{env, fs, path::Path, time::Instant};

mod measurement;
#[cfg(feature = "candidate")]
#[path = "../../../../crates/katana-render-runtime/examples/html_fidelity_probe.rs"]
mod native_frame_capture;
mod operations;
mod options;
#[path = "../../../../crates/katana-render-runtime/src/system/process.rs"]
mod process;
mod provenance;
#[cfg(test)]
#[path = "harness_test_support.rs"]
mod test_support;

use measurement::measure_one;
use options::{parse_options, worker_sample, Options};
use process::ProcessService;
use provenance::{arm_name, hex_digest, provenance};

const SAMPLE_COUNT: usize = 5;
const MILLISECONDS_PER_SECOND: f64 = 1_000.0;
type PhaseSnapshot = serde_json::Map<String, serde_json::Value>;
#[cfg(target_os = "macos")]
const BYTES_PER_RSS_UNIT: u64 = 1;
#[cfg(not(target_os = "macos"))]
const BYTES_PER_RSS_UNIT: u64 = 1024;

fn measure_samples(
    options: &Options,
    source_hash: &str,
    viewport: HtmlBrowserViewport,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let executable = env::current_exe()?;
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        samples.push(measure_sample_worker(&executable, options, sample)?);
    }
    let pair_index: usize = env::var("KRR_PERF_PAIR_INDEX")?.parse()?;
    let measurement_executed = env::var("KRR_PERF_MEASUREMENT_EXECUTED")? == "1";
    Ok(serde_json::json!({
        "arm": arm_name(),
        "pair_index": pair_index,
        "measurement_executed": measurement_executed,
        "sample_count": samples.len(),
        "source_sha256": source_hash,
        "operation_plan_sha256": hex_digest(&fs::read(&options.operation_plan_path)?),
        "operation": options.operation_plan.operation(),
        "expected_dom_state_sha256": options.operation_plan.expected_dom_state_sha256(),
        "catalog_condition": catalog_condition(&options.operation_plan),
        "viewport": {"width": viewport.width, "height": viewport.height, "scale": viewport.device_scale_factor},
        "provenance": provenance()?,
        "samples": samples,
    }))
}

fn catalog_condition(operation_plan: &options::OperationPlan) -> serde_json::Value {
    match operation_plan {
        options::OperationPlan::Legacy(_) => serde_json::Value::Null,
        options::OperationPlan::Catalog(plan) => serde_json::json!({
            "profile": plan.profile,
            "condition_id": plan.condition_id,
            "case_id": plan.case_id,
            "state": plan.state,
            "operation_kind": plan.operation_kind,
            "target_selector": plan.target_selector,
            "operation_evidence_path": plan.operation_evidence_path,
            "operation_evidence_sha256": plan.operation_evidence_sha256,
            "evidence_binary_sha256": plan.evidence_binary_sha256,
        }),
    }
}

fn measure_sample_worker(
    executable: &Path,
    options: &Options,
    sample: usize,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let output = ProcessService::create_command(executable)
        .arg("--worker")
        .arg(sample.to_string())
        .arg(&options.input)
        .arg(&options.output)
        .arg(&options.operation_plan_path)
        .arg(options.width.to_string())
        .arg(options.height.to_string())
        .arg(options.scale.to_string())
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "worker {sample} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice::<serde_json::Value>(&output.stdout)?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "candidate")]
    if env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--input")) {
        native_frame_capture::main();
        return Ok(());
    }
    run_measurement()
}

fn run_measurement() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1).peekable();
    if let Some(sample) = worker_sample(&mut args)? {
        let options = parse_options(args)?;
        let html = fs::read(&options.input)?;
        let source_hash = hex_digest(&html);
        let viewport = HtmlBrowserViewport::new(options.width, options.height, options.scale)?;
        let sample = measure_one(
            &html,
            &source_hash,
            viewport,
            sample,
            &options.operation_plan,
        )?;
        println!("{}", serde_json::to_string(&sample)?);
        return Ok(());
    }
    let options = parse_options(args)?;
    let html = fs::read(&options.input)?;
    let source_hash = hex_digest(&html);
    let viewport = HtmlBrowserViewport::new(options.width, options.height, options.scale)?;
    let measurements = measure_samples(&options, &source_hash, viewport)?;
    fs::write(options.output, serde_json::to_vec_pretty(&measurements)?)?;
    Ok(())
}

fn phase_snapshot(
    start: Instant,
    session: &impl SessionFrame,
) -> Result<PhaseSnapshot, Box<dyn std::error::Error>> {
    let elapsed_ms = start.elapsed().as_secs_f64() * MILLISECONDS_PER_SECOND;
    let peak_rss_bytes = peak_rss_bytes()?;
    let frame = session.frame_state()?;
    Ok(serde_json::Map::from_iter([
        ("elapsed_ms".to_owned(), serde_json::json!(elapsed_ms)),
        (
            "peak_rss_bytes".to_owned(),
            serde_json::json!(peak_rss_bytes),
        ),
        (
            "peak_rss_scope".to_owned(),
            serde_json::json!("process-cumulative-including-phase-setup"),
        ),
        ("frame".to_owned(), frame),
    ]))
}

trait SessionFrame {
    fn frame_state(&self) -> Result<serde_json::Value, Box<dyn std::error::Error>>;
}

impl SessionFrame for HtmlBrowserSession {
    fn frame_state(&self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let frame = self.latest_frame().ok_or("missing session frame")?;
        Ok(
            serde_json::json!({"generation": frame.generation, "scroll_y": frame.scroll_y,
            "content_height": frame.content_height, "pixel_bytes": frame.pixels.len(),
            "origin": frame.origin.as_str(),
             "viewport": {"width": frame.viewport.width, "height": frame.viewport.height, "scale": frame.viewport.device_scale_factor}}),
        )
    }
}

#[cfg(feature = "baseline")]
type HtmlBrowserSession = baseline::HtmlBrowserSession;
#[cfg(feature = "candidate")]
type HtmlBrowserSession = candidate::HtmlBrowserSession;

fn peak_rss_bytes() -> Result<u64, Box<dyn std::error::Error>> {
    /* WHY: 同じgetrusageのピークRSSを保ち、ABIとOSエラー処理は既存ライブラリへ委譲する。 */
    let usage = nix::sys::resource::getrusage(nix::sys::resource::UsageWho::RUSAGE_SELF)?;
    let value = u64::try_from(usage.max_rss())?;
    Ok(value.saturating_mul(BYTES_PER_RSS_UNIT))
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
