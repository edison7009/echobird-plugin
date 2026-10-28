//! Background commands share Windows console suppression. User-requested
//! interactive terminal launches stay in process_manager.
use std::{ffi::OsStr, process::Command};

pub fn command(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)] // Windows adds creation flags; other platforms keep std defaults.
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

pub fn async_command(program: impl AsRef<OsStr>) -> tokio::process::Command {
    command(program).into()
}

#[cfg(windows)]
pub fn owned_process_tree(
    child: &tokio::process::Child,
) -> Result<std::os::windows::io::OwnedHandle, String> {
    use std::os::windows::io::FromRawHandle;
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
    };
    let process = child
        .raw_handle()
        .ok_or("Child process handle is unavailable")?;
    unsafe {
        let job = CreateJobObjectW(None, None).map_err(|_| "Could not create child process job")?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        )
        .and_then(|_| AssignProcessToJobObject(job, HANDLE(process)));
        if configured.is_err() {
            let _ = CloseHandle(job);
            return Err("Could not own child process tree".into());
        }
        Ok(std::os::windows::io::OwnedHandle::from_raw_handle(job.0))
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[tokio::test]
    async fn background_children_have_no_console_and_preserve_output_and_status() {
        let script = "Add-Type 'using System; using System.Runtime.InteropServices; public class Probe { [DllImport(\"kernel32.dll\")] public static extern IntPtr GetConsoleWindow(); }'; [Probe]::GetConsoleWindow().ToInt64(); exit 7";
        let args = ["-NoProfile", "-NonInteractive", "-Command", script];
        let sync = super::command("powershell.exe")
            .args(args)
            .output()
            .unwrap();
        let asynchronous = super::async_command("powershell.exe")
            .args(args)
            .output()
            .await
            .unwrap();
        for result in [sync, asynchronous] {
            assert_eq!(result.status.code(), Some(7));
            assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "0");
        }
    }
}
