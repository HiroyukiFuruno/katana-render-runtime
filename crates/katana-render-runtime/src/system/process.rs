use std::{ffi::OsStr, process::Command};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) struct ProcessService;

impl ProcessService {
    pub(crate) fn create_command(program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        Self::apply_platform_defaults(&mut command);
        command
    }

    #[cfg(test)]
    pub(crate) fn enter_isolated_test_child(
        environment_key: &str,
        case_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let child_args = ["--exact", case_name, "--test-threads=1"];
        if let Some(child_case) = std::env::var_os(environment_key) {
            assert_eq!(child_case, case_name);
            assert!(std::env::args().skip(1).eq(child_args));
            return Ok(true);
        }

        let output = Self::create_command(std::env::current_exe()?)
            .args(child_args)
            .env(environment_key, case_name)
            .output()?;
        let stdout = std::str::from_utf8(&output.stdout)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{case_name}: {stdout}\n{stderr}");
        assert!(
            stdout.contains("test result: ok. 1 passed;"),
            "{case_name}: {stdout}"
        );
        Ok(false)
    }

    #[cfg(windows)]
    fn apply_platform_defaults(command: &mut Command) {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    #[cfg(not(windows))]
    fn apply_platform_defaults(_command: &mut Command) {}
}

#[cfg(test)]
mod tests {
    use super::ProcessService;

    #[test]
    fn creates_a_command_for_the_requested_program() {
        let command = ProcessService::create_command("krr-test-command");

        assert_eq!(command.get_program(), "krr-test-command");
    }
}
