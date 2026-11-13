use super::ProcessService;

impl ProcessService {
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
}
