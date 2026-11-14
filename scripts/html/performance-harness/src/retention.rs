use std::io::{self, BufRead, Write};

use super::{hex_digest, measure_one, options::parse_options, HtmlBrowserViewport};

const WARMUP_CYCLES: usize = 5;
const MEASUREMENT_CYCLES: usize = 25;
const SAMPLES_PER_PAIR: usize = 5;

pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_options(std::env::args_os().skip(2))?;
    let html = std::fs::read(&options.input)?;
    let source_hash = hex_digest(&html);
    let viewport = HtmlBrowserViewport::new(options.width, options.height, options.scale)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    for cycle in 0..(WARMUP_CYCLES + MEASUREMENT_CYCLES) {
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
