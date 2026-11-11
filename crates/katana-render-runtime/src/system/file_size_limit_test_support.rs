use super::ProcessService;
use nix::sys::resource::{Resource, getrlimit, setrlimit};

pub(crate) struct FileSizeLimitTest;

impl FileSizeLimitTest {
    pub(crate) fn run_in_child(
        module: &str,
        case: &str,
        run: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        const CHILD: &str = "KRR_FILE_SIZE_LIMIT_TEST_CHILD";
        if std::env::var(CHILD).as_deref() == Ok("1") {
            return run();
        }
        let (_, module) = module.split_once("::").ok_or("test module missing")?;
        let name = format!("{module}::{case}");
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let output = ProcessService::create_command("/bin/sh")
            .args([
                "-c",
                "trap '' XFSZ && exec \"$@\"",
                "file-size-limit-test-child",
            ])
            .arg(executable)
            .args(["--exact", name.as_str(), "--color=never"])
            .env(CHILD, "1")
            .output()
            .map_err(|error| error.to_string())?;
        Self::assert_child_passed(&name, &output);
        Ok(())
    }

    fn assert_child_passed(name: &str, output: &std::process::Output) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let passed = format!("test {name} ... ok");
        assert_eq!(stdout.lines().filter(|line| *line == passed).count(), 1);
    }

    pub(crate) fn with_zero_limit<T>(run: impl FnOnce() -> T) -> Result<T, String> {
        let limit = Resource::RLIMIT_FSIZE;
        let (soft, hard) = getrlimit(limit).map_err(|error| error.to_string())?;
        setrlimit(limit, 0, hard).map_err(|error| error.to_string())?;
        let result = run();
        /* WHY: LLVMのプロファイルを書き込めるよう、子プロセスが終了する前に制限を戻す。 */
        setrlimit(limit, soft, hard).map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub(crate) fn limit_error() -> std::io::Error {
        std::io::Error::from_raw_os_error(nix::errno::Errno::EFBIG as i32)
    }
}
