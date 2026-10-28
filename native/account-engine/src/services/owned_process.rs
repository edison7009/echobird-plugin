/// A temporary native client used only by an explicit configuration operation.
/// It does not enter the normal launcher cooldown or restart an existing client.
pub(crate) struct OwnedDesktopConfigProcess {
    child: std::process::Child,
    stopped: bool,
}

#[cfg(windows)]
pub(crate) fn owned_windows_process_times(
    child: &impl std::os::windows::io::AsRawHandle,
) -> Result<(u64, u64), String> {
    use windows::Win32::Foundation::FILETIME;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetProcessTimes(
            process: std::os::windows::io::RawHandle,
            created: *mut FILETIME,
            exited: *mut FILETIME,
            kernel: *mut FILETIME,
            user: *mut FILETIME,
        ) -> i32;
    }
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // This is the original Child handle, including after exit, not a PID lookup.
    if unsafe {
        GetProcessTimes(
            child.as_raw_handle(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    } == 0
    {
        return Err("accountError.closeClient".into());
    }
    let micros = |time: FILETIME| {
        ((u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)) / 10
    };
    Ok((micros(created), micros(exited)))
}

#[cfg(windows)]
pub(crate) fn stop_owned_windows_tree(pid: u32, created: u64, exited: u64) -> Result<(), String> {
    let script = r#"
$ErrorActionPreference='Stop'
function Created($entry){return [long][decimal]::Truncate(([decimal]($entry.CreationDate.ToFileTimeUtc()))/10)}
$all=@(Get-CimInstance Win32_Process -ErrorAction Stop)
$root=$all | Where-Object { $_.ProcessId -eq [uint32]$env:ECHOBIRD_OWNED_CONFIG_PID }
if($root -and (Created $root) -ne [long]$env:ECHOBIRD_OWNED_CONFIG_CREATED){exit 2}
$queue=[Collections.Generic.Queue[object]]::new()
$queue.Enqueue(@{Pid=[uint32]$env:ECHOBIRD_OWNED_CONFIG_PID;Created=[long]$env:ECHOBIRD_OWNED_CONFIG_CREATED;Exited=[long]$env:ECHOBIRD_OWNED_CONFIG_EXITED})
$owned=@()
while($queue.Count -gt 0){
  $parent=$queue.Dequeue()
  foreach($entry in $all | Where-Object { $_.ParentProcessId -eq $parent.Pid }){
    $birth=Created $entry
    if($birth -lt $parent.Created -or ($parent.Exited -gt 0 -and $birth -gt $parent.Exited)){continue}
    $identity=@{Pid=$entry.ProcessId;Created=$birth;Exited=0}
    $owned+=$identity
    $queue.Enqueue($identity)
  }
}
if($root){$owned+=@{Pid=$root.ProcessId;Created=(Created $root)}}
$ErrorActionPreference='Continue'
if($root){& taskkill.exe /PID $root.ProcessId /T /F >$null 2>&1}
$survivors=@(Get-CimInstance Win32_Process -ErrorAction Stop)
foreach($entry in $owned){
  $current=$survivors | Where-Object { $_.ProcessId -eq $entry.Pid }
  if($current -and (Created $current) -eq $entry.Created){& taskkill.exe /PID $entry.Pid /T /F >$null 2>&1}
}
$ErrorActionPreference='Stop'
$deadline=[DateTime]::UtcNow.AddSeconds(3)
do{
  $remaining=$false
  $current=@(Get-CimInstance Win32_Process -ErrorAction Stop)
  foreach($entry in $owned){
    $candidate=$current | Where-Object { $_.ProcessId -eq $entry.Pid }
    if($candidate -and (Created $candidate) -eq $entry.Created){$remaining=$true}
  }
  if(-not $remaining){exit 0}
  Start-Sleep -Milliseconds 100
}while([DateTime]::UtcNow -lt $deadline)
exit 1
"#;
    let status = crate::utils::process::command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("ECHOBIRD_OWNED_CONFIG_PID", pid.to_string())
        .env("ECHOBIRD_OWNED_CONFIG_CREATED", created.to_string())
        .env("ECHOBIRD_OWNED_CONFIG_EXITED", exited.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| "accountError.closeClient")?;
    if status.success() {
        Ok(())
    } else {
        Err("accountError.closeClient".into())
    }
}

impl OwnedDesktopConfigProcess {
    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn stop(&mut self) -> Result<(), String> {
        if self.stopped {
            return Ok(());
        }
        #[cfg(windows)]
        {
            // taskkill /T can terminate the parent while a native SDK child
            // survives. Capture only this owned tree before killing it, and
            // retain creation times so a reused descendant PID is never targeted.
            let (created, ended) = owned_windows_process_times(&self.child)?;
            stop_owned_windows_tree(self.id(), created, ended)?;
        }
        #[cfg(unix)]
        {
            // Keep the leader unreaped until its owned helpers are stopped: an
            // exited leader can leave helpers alive, and its PID must not be reused.
            if unsafe { libc::kill(-(self.id() as i32), libc::SIGKILL) } != 0
                && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
            {
                return Err("accountError.closeClient".into());
            }
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if self
                .child
                .try_wait()
                .map_err(|_| "accountError.closeClient")?
                .is_some()
            {
                self.stopped = true;
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err("accountError.closeClient".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

impl Drop for OwnedDesktopConfigProcess {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(crate) fn start_desktop_for_config(tool_id: &str) -> Result<OwnedDesktopConfigProcess, String> {
    start_desktop_with_config_args(tool_id, &[])
}

pub(crate) fn start_doubao_account_bridge(
    tool_id: &str,
    port: u16,
) -> Result<OwnedDesktopConfigProcess, String> {
    let product = match tool_id {
        "doubaowork" => "DoubaoWork",
        "dola" => "Cici",
        _ => return Err("accountError.format".into()),
    };
    if port == 0 {
        return Err("accountError.format".into());
    }
    #[cfg(windows)]
    let data_dir = dirs::data_local_dir().map(|root| root.join(product).join("User Data"));
    #[cfg(target_os = "macos")]
    let data_dir = dirs::data_dir().map(|root| root.join(product));
    #[cfg(not(any(windows, target_os = "macos")))]
    let data_dir = dirs::config_dir().map(|root| root.join(product));
    let data_dir = data_dir.ok_or("accountError.initializeClient")?;
    start_desktop_with_config_args(
        tool_id,
        &[
            "--headless=new".into(),
            format!("--user-data-dir={}", data_dir.display()),
            "--remote-debugging-address=127.0.0.1".into(),
            format!("--remote-debugging-port={port}"),
        ],
    )
}

#[cfg(test)]
pub(crate) fn start_doubao_account_restart_probe(
    port: u16,
) -> Result<OwnedDesktopConfigProcess, String> {
    start_desktop_with_config_args(
        "doubaowork",
        &[
            "--remote-debugging-address=127.0.0.1".into(),
            format!("--remote-debugging-port={port}"),
        ],
    )
}

fn start_desktop_with_config_args(
    tool_id: &str,
    arguments: &[String],
) -> Result<OwnedDesktopConfigProcess, String> {
    let executable =
        super::tool_manager::get_tool_exe_path(tool_id).ok_or("accountError.initializeClient")?;
    let mut command = crate::utils::process::command(&executable);
    command.args(arguments);
    if let Some(directory) = std::path::Path::new(&executable).parent() {
        command.current_dir(directory);
    }
    command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command
        .spawn()
        .map_err(|_| "accountError.initializeClient")?;
    Ok(OwnedDesktopConfigProcess {
        child,
        stopped: false,
    })
}
