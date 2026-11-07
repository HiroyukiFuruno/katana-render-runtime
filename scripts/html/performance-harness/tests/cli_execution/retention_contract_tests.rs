#![cfg(test)]

use super::{assert_sample, create_temp_dir, valid_inputs, Inputs, BIN, HEIGHT, WIDTH};
use serde_json::Value;
use std::{
    error::Error,
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
};

type TestResult<T> = Result<T, Box<dyn Error>>;
const TOTAL_CYCLES: usize = 30;
const SAMPLES_PER_PAIR: usize = 5;

struct Probe {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl Probe {
    fn start(inputs: &Inputs, directory: &Path) -> TestResult<Self> {
        let child = Command::new(BIN)
            .arg("--retention")
            .arg(&inputs.fixture)
            .arg(directory.join("unused.json"))
            .arg(&inputs.plan)
            .arg(WIDTH.to_string())
            .arg(HEIGHT.to_string())
            .arg("1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(File::create(
                directory.join("retention.stderr"),
            )?))
            .spawn()?;
        Self::attach(child)
    }

    fn attach(mut child: Child) -> TestResult<Self> {
        let input = child.stdin.take();
        let Some(output) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("retention stdout missing".into());
        };
        if input.is_none() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("retention stdin missing".into());
        }
        Ok(Self {
            child,
            input,
            output: BufReader::new(output),
        })
    }

    fn read_line(&mut self) -> TestResult<String> {
        let mut line = String::new();
        if self.output.read_line(&mut line)? == 0 {
            return Err("retention stdout ended early".into());
        }
        Ok(line)
    }

    fn acknowledge(&mut self, value: &str) -> TestResult<()> {
        let input = self.input.as_mut().ok_or("retention stdin closed")?;
        input.write_all(value.as_bytes())?;
        input.flush()?;
        Ok(())
    }

    fn finish(&mut self) -> TestResult<ExitStatus> {
        self.input.take();
        Ok(self.child.wait()?)
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.input.take();
        if matches!(self.child.try_wait(), Ok(None) | Err(_)) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn read_cycle(probe: &mut Probe, cycle: usize, inputs: &Inputs) -> TestResult<()> {
    let line = probe.read_line()?;
    let prefix = format!("SAMPLE {cycle} ");
    let raw = line
        .strip_prefix(&prefix)
        .ok_or("retention sample order differs")?;
    let sample: Value = serde_json::from_str(raw)?;
    assert_sample(&sample, cycle % SAMPLES_PER_PAIR, &inputs.fixture_sha)?;
    assert_eq!(probe.read_line()?, format!("READY {cycle}\n"));
    Ok(())
}

#[test]
fn real_retention_worker_closes_thirty_valid_sessions() -> TestResult<()> {
    let temp = create_temp_dir()?;
    let inputs = valid_inputs(&temp)?;
    let mut probe = Probe::start(&inputs, temp.path())?;
    for cycle in 0..TOTAL_CYCLES {
        read_cycle(&mut probe, cycle, &inputs)?;
        probe.acknowledge(&format!("ACK {cycle}\n"))?;
    }
    assert!(probe.finish()?.success());
    let mut tail = String::new();
    assert_eq!(probe.output.read_line(&mut tail)?, 0);
    assert!(!temp.path().join("unused.json").exists());
    Ok(())
}

#[test]
fn real_retention_worker_rejects_eof_and_incorrect_acknowledgement() -> TestResult<()> {
    for wrong_ack in [false, true] {
        let temp = create_temp_dir()?;
        let inputs = valid_inputs(&temp)?;
        let mut probe = Probe::start(&inputs, temp.path())?;
        read_cycle(&mut probe, 0, &inputs)?;
        if wrong_ack {
            probe.acknowledge("ACK 1\n")?;
        }
        assert!(!probe.finish()?.success());
        assert!(fs::read_to_string(temp.path().join("retention.stderr"))?
            .contains("retention probe acknowledgement differs"));
    }
    Ok(())
}

#[test]
fn real_retention_worker_propagates_post_operation_failure() -> TestResult<()> {
    let temp = create_temp_dir()?;
    let inputs = valid_inputs(&temp)?;
    let mut plan: Value = serde_json::from_slice(&fs::read(&inputs.plan)?)?;
    plan["expected_dom_state_sha256"] = "0".repeat(64).into();
    fs::write(&inputs.plan, serde_json::to_vec(&plan)?)?;
    let mut probe = Probe::start(&inputs, temp.path())?;
    assert!(!probe.finish()?.success());
    assert!(fs::read_to_string(temp.path().join("retention.stderr"))?
        .contains("expected DOM state evidence SHA does not match its file"));
    Ok(())
}
