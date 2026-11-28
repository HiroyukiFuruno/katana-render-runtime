use super::ProcessService;
use nix::sys::resource::{Resource, getrlimit, setrlimit};

pub(crate) struct FileSizeLimitTest;

impl FileSizeLimitTest {
    pub(crate) fn run_in_child(
        module: &str,
        case: &str,
        run: impl FnOnce() -> Result<(), Box<dyn std::error::Error>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        const CHILD: &str = "KRR_FILE_SIZE_LIMIT_TEST_CHILD";
        if std::env::var(CHILD).as_deref() == Ok("1") {
            return run();
        }
        let name = Self::test_name(module, case);
        let executable = std::env::current_exe()?;
        let output = ProcessService::create_command("/bin/sh")
            .args([
                "-c",
                "trap '' XFSZ && exec \"$@\"",
                "file-size-limit-test-child",
            ])
            .arg(executable)
            .args(["--exact", name.as_str(), "--color=never"])
            .env(CHILD, "1")
            .output()?;
        Self::assert_child_passed(&name, &output);
        Ok(())
    }

    fn test_name(module: &str, case: &str) -> String {
        module
            .split("::")
            .skip(1)
            .chain(std::iter::once(case))
            .collect::<Vec<_>>()
            .join("::")
    }

    fn assert_child_passed(name: &str, output: &std::process::Output) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        let passed = format!("test {name} ... ok");
        assert_eq!(stdout.lines().filter(|line| *line == passed).count(), 1);
    }

    pub(crate) fn with_zero_limit<T>(run: impl FnOnce() -> T) -> nix::Result<T> {
        let limit = Resource::RLIMIT_FSIZE;
        let (soft, hard) = getrlimit(limit)?;
        setrlimit(limit, 0, hard)?;
        let result = run();
        /* WHY: LLVMのプロファイルを書き込めるよう、子プロセスが終了する前に制限を戻す。 */
        setrlimit(limit, soft, hard)?;
        Ok(result)
    }

    pub(crate) fn limit_error() -> std::io::Error {
        std::io::Error::from_raw_os_error(nix::errno::Errno::EFBIG as i32)
    }
}
