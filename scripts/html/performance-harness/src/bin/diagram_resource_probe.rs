#[cfg(all(feature = "baseline", feature = "candidate"))]
compile_error!("select exactly one measurement arm");
#[cfg(not(any(feature = "baseline", feature = "candidate")))]
compile_error!("select the baseline or candidate measurement arm");

#[cfg(feature = "baseline")]
use baseline as runtime;
#[cfg(feature = "candidate")]
use candidate as runtime;
use runtime::{DiagramKind, DrawioRenderer, MermaidRenderer, RenderInput, RenderOutput, Renderer};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    hint::black_box,
    path::{Path, PathBuf},
    time::Instant,
};

#[path = "../rss.rs"]
mod rss;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Options {
    kind: Kind,
    input: PathBuf,
    runtime: PathBuf,
    repetitions: usize,
}

#[derive(Clone, Copy)]
enum Kind {
    Mermaid,
    Drawio,
}

fn main() -> Result<()> {
    let options = parse_options(std::env::args().skip(1).collect())?;
    let source = fs::read_to_string(&options.input)?;
    let runtime_digest = digest(&fs::read(&options.runtime)?);
    let input = RenderInput {
        kind: options.kind.diagram_kind(),
        source,
        config: Default::default(),
        policy: Default::default(),
        context: Default::default(),
    };
    let renderer = renderer(&options);
    let initial_seconds = measure(&*renderer, &input)?;
    let warm_seconds = measure_warm(&*renderer, &input, options.repetitions)?;
    verify_runtime_digest(&options.runtime, &runtime_digest)?;
    print_result(
        &options,
        &input,
        &runtime_digest,
        initial_seconds,
        warm_seconds,
    )
}

fn print_result(
    options: &Options,
    input: &RenderInput,
    runtime_digest: &str,
    initial_seconds: f64,
    warm_seconds: Vec<f64>,
) -> Result<()> {
    let warm_count = warm_seconds.len();
    println!(
        "{}",
        serde_json::json!({
            "schema": "krr-diagram-resource-probe-v1",
            "kind": options.kind.name(),
            "input_sha256": digest(input.source.as_bytes()),
            "runtime_sha256": runtime_digest,
            "initial_seconds": initial_seconds,
            "warm_seconds": warm_seconds,
            "warm_count": warm_count,
        "peak_rss_bytes": rss::ProcessRss::peak_bytes()?,
            "peak_rss_scope": "process-lifetime",
        })
    );
    Ok(())
}

fn parse_options(args: Vec<String>) -> Result<Options> {
    let [kind, input, runtime, repetitions] = args.as_slice() else {
        return Err(
            "usage: diagram_resource_probe <mermaid|drawio> <input> <runtime> <warm-count>".into(),
        );
    };
    let kind = match kind.as_str() {
        "mermaid" => Kind::Mermaid,
        "drawio" => Kind::Drawio,
        _ => return Err("kind must be mermaid or drawio".into()),
    };
    let repetitions = repetitions.parse()?;
    if repetitions == 0 {
        return Err("warm-count must be positive".into());
    }
    Ok(Options {
        kind,
        input: input.into(),
        runtime: runtime.into(),
        repetitions,
    })
}

fn renderer(options: &Options) -> Box<dyn Renderer> {
    match options.kind {
        Kind::Mermaid => Box::new(MermaidRenderer::with_runtime_path(options.runtime.clone())),
        Kind::Drawio => Box::new(DrawioRenderer::with_runtime_path(options.runtime.clone())),
    }
}

fn measure(renderer: &dyn Renderer, input: &RenderInput) -> Result<f64> {
    let start = Instant::now();
    let output = renderer.render(input)?;
    let elapsed = start.elapsed().as_secs_f64();
    require_clean_svg(&output)?;
    black_box(output);
    Ok(elapsed)
}

fn require_clean_svg(output: &RenderOutput) -> Result<()> {
    if !output.diagnostics.errors.is_empty()
        || !output.diagnostics.warnings.is_empty()
        || !output.svg.contains("<svg")
    {
        return Err("render did not produce a clean SVG".into());
    }
    Ok(())
}

fn measure_warm(renderer: &dyn Renderer, input: &RenderInput, count: usize) -> Result<Vec<f64>> {
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        samples.push(measure(renderer, input)?);
    }
    Ok(samples)
}

fn verify_runtime_digest(path: &Path, expected: &str) -> Result<()> {
    if digest(&fs::read(path)?) != expected {
        return Err("runtime changed during measurement".into());
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Mermaid => "mermaid",
            Self::Drawio => "drawio",
        }
    }

    fn diagram_kind(self) -> DiagramKind {
        match self {
            Self::Mermaid => DiagramKind::Mermaid,
            Self::Drawio => DiagramKind::Drawio,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::runtime::{RenderDiagnostics, RenderOutput, RendererProfile, RuntimeVersion};
    use super::{digest, parse_options, require_clean_svg, verify_runtime_digest};
    use std::fs;

    #[test]
    fn runtime_digest_rejects_changed_contents() -> super::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("runtime.js");
        fs::write(&path, b"before")?;
        let expected = digest(b"before");
        assert!(verify_runtime_digest(&path, &expected).is_ok());
        fs::write(&path, b"after")?;
        let error = verify_runtime_digest(&path, &expected)
            .err()
            .map(|error| error.to_string());
        assert_eq!(error.as_deref(), Some("runtime changed during measurement"));
        Ok(())
    }

    #[test]
    fn svg_rejects_each_unclean_result() {
        let mut output = RenderOutput {
            svg: "<svg></svg>".into(),
            width: 1.0,
            height: 1.0,
            view_box: String::new(),
            runtime: RuntimeVersion {
                name: String::new(),
                version: String::new(),
                checksum: None,
            },
            profile: RendererProfile {
                id: String::new(),
                description: None,
            },
            diagnostics: RenderDiagnostics {
                warnings: Vec::new(),
                errors: Vec::new(),
            },
            cache_fingerprint: String::new(),
        };
        assert!(require_clean_svg(&output).is_ok());
        output.diagnostics.errors.push("error".into());
        assert!(require_clean_svg(&output).is_err());
        output.diagnostics.errors.clear();
        output.diagnostics.warnings.push("warning".into());
        assert!(require_clean_svg(&output).is_err());
        output.diagnostics.warnings.clear();
        output.svg.clear();
        assert!(require_clean_svg(&output).is_err());
    }

    #[test]
    fn options_require_supported_kind_and_positive_count() {
        let args = |kind: &str, count: &str| {
            [kind, "input", "runtime", count]
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        assert!(parse_options(args("mermaid", "1")).is_ok());
        assert!(parse_options(args("drawio", "2")).is_ok());
        assert!(parse_options(args("mathjax", "1")).is_err());
        assert!(parse_options(args("mermaid", "0")).is_err());
        assert!(parse_options(args("mermaid", "not-a-number")).is_err());
        assert!(parse_options(Vec::new()).is_err());
        let mut extra = args("mermaid", "1");
        extra.push("unexpected".to_owned());
        assert!(parse_options(extra).is_err());
    }
}
