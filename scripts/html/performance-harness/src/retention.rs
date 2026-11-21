use std::io::{self, BufRead, Write};

use super::{hex_digest, measure_one, options::parse_options, HtmlBrowserViewport};

const WARMUP_CYCLES: usize = 5;
const MEASUREMENT_CYCLES: usize = 25;
const DIAGNOSTIC_CYCLES: usize = 100;
const SAMPLES_PER_PAIR: usize = 5;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Standard,
    Diagnostic,
}

pub(super) fn mode(argument: Option<&std::ffi::OsStr>) -> Option<Mode> {
    match argument.and_then(std::ffi::OsStr::to_str) {
        Some("--retention") => Some(Mode::Standard),
        Some("--retention-diagnostic") => Some(Mode::Diagnostic),
        _ => None,
    }
}

impl Mode {
    fn total_cycles(&self) -> usize {
        WARMUP_CYCLES
            + match self {
                Self::Standard => MEASUREMENT_CYCLES,
                Self::Diagnostic => DIAGNOSTIC_CYCLES,
            }
    }
}

pub(super) fn run(mode: Mode) -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_options(std::env::args_os().skip(2))?;
    let html = std::fs::read(&options.input)?;
    let source_hash = hex_digest(&html);
    let viewport = HtmlBrowserViewport::new(options.width, options.height, options.scale)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    for cycle in 0..mode.total_cycles() {
        let sample = measure_one(
            &html,
            &source_hash,
            viewport,
            cycle % SAMPLES_PER_PAIR,
            &options.operation_plan,
        )?;
        write!(output, "SAMPLE {cycle} ")?;
        serde_json::to_writer(&mut output, &sample)?;
        writeln!(output)?;
        output.flush()?;
        drop(sample);
        writeln!(output, "READY {cycle}")?;
        output.flush()?;
        require_ack(&mut input, cycle)?;
    }
    Ok(())
}

fn require_ack(input: &mut impl BufRead, cycle: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 || line != format!("ACK {cycle}\n") {
        return Err("retention probe acknowledgement differs".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
