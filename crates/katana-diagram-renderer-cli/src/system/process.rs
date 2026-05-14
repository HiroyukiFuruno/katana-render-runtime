use std::ffi::OsStr;
use std::process::Command;

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

    #[cfg(windows)]
    fn apply_platform_defaults(command: &mut Command) {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    #[cfg(not(windows))]
    fn apply_platform_defaults(_command: &mut Command) {}
}
