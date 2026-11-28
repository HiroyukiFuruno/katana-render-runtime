use std::{
    ffi::OsStr,
    io::Read,
    process::{Child, ExitStatus, Stdio},
    thread::JoinHandle,
};

const CHILD_CASE: &str = "KRR_USVG_CMAP_GLOBAL_TEST_CHILD_CASE";

pub(in super::super::super) fn run_parent_or_allow_child(name: &str) -> Result<bool, String> {
    let flag = std::env::var_os(CHILD_CASE);
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if child_authorized(name, flag.as_deref(), &args)? {
        return Ok(true);
    }
    assert!(!super::cache().is_poisoned());
    let output = run_child(name)?;
    validate_case_output(name, &output)?;
    assert!(!super::cache().is_poisoned());
    Ok(false)
}

fn child_authorized(name: &str, flag: Option<&OsStr>, args: &[String]) -> Result<bool, String> {
    let Some(flag) = flag else { return Ok(false) };
    let expected = ["--exact", name, "--test-threads=1", "--nocapture"];
    if flag != OsStr::new(name) || !args.iter().map(String::as_str).eq(expected) {
        return Err("global-cache child flag and exact single-case argv must agree".into());
    }
    Ok(true)
}

struct OwnedChild(Option<Child>);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct CaseOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn run_child(name: &str) -> Result<CaseOutput, String> {
    let child =
        crate::system::ProcessService::create_command(std::env::current_exe().map_err(to_string)?)
            .args(["--exact", name, "--test-threads=1", "--nocapture"])
            .env(CHILD_CASE, name)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(to_string)?;
    let mut owned = OwnedChild(Some(child));
    let child = owned.0.as_mut().ok_or("child missing")?;
    let stdout = read_pipe(child.stdout.take().ok_or("stdout pipe missing")?);
    let stderr = read_pipe(child.stderr.take().ok_or("stderr pipe missing")?);
    let status = wait_child(&mut owned);
    drop(owned);
    let stdout = join_pipe(stdout);
    let stderr = join_pipe(stderr);
    Ok(CaseOutput {
        status: status?,
        stdout: stdout?,
        stderr: stderr?,
    })
}

fn wait_child(owned: &mut OwnedChild) -> Result<ExitStatus, String> {
    let child = owned.0.as_mut().ok_or("child missing")?;
    match child.wait() {
        Ok(status) => {
            owned.0.take();
            Ok(status)
        }
        Err(error) => {
            let killed = child.kill();
            let reaped = child.wait();
            Err(format!(
                "wait failed: {error}; kill: {killed:?}; reap: {reaped:?}"
            ))
        }
    }
}

fn read_pipe(mut pipe: impl Read + Send + 'static) -> JoinHandle<Result<Vec<u8>, String>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).map_err(to_string)?;
        Ok(bytes)
    })
}

fn join_pipe(reader: JoinHandle<Result<Vec<u8>, String>>) -> Result<String, String> {
    let bytes = reader.join().map_err(|_| "child pipe reader panicked")??;
    String::from_utf8(bytes).map_err(to_string)
}

fn validate_case_output(name: &str, output: &CaseOutput) -> Result<(), String> {
    let case = format!("test {name} ... ok");
    let summary = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ";
    if !output.status.success()
        || output.stdout.lines().filter(|line| *line == case).count() != 1
        || output
            .stdout
            .lines()
            .filter(|line| line.starts_with(summary))
            .count()
            != 1
        || !output.stdout.lines().any(|line| line == "running 1 test")
    {
        return Err(format!(
            "global-cache child did not pass exactly one case: {:?}\n{}\n{}",
            output.status, output.stdout, output.stderr
        ));
    }
    Ok(())
}

fn to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[test]
fn child_gate_rejects_flag_and_argv_escape() -> Result<(), String> {
    let name = "one::case";
    let args = ["--exact", name, "--test-threads=1", "--nocapture"].map(String::from);
    assert!(!child_authorized(name, None, &args)?);
    assert!(child_authorized(name, Some(OsStr::new(name)), &args)?);
    assert!(child_authorized(name, Some(OsStr::new("wrong")), &args).is_err());
    for index in 0..args.len() {
        let mut wrong = args.clone();
        wrong[index] = "wrong".into();
        assert!(child_authorized(name, Some(OsStr::new(name)), &wrong).is_err());
    }
    let mut extra = args.to_vec();
    extra.push("--ignored".into());
    assert!(child_authorized(name, Some(OsStr::new(name)), &extra).is_err());
    Ok(())
}
