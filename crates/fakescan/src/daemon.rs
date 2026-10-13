//! `fakescan daemon install | uninstall | status`: a thin wrapper over a per-user **logon
//! scheduled task** that runs `fakescan --manage`. Not a Windows service: the daemon needs no
//! elevation, its registry lives in `%LOCALAPPDATA%`, and Imaging Add is per-user.
//!
//! Implemented with `schtasks.exe`; on other platforms every action reports "Windows only".

use std::path::Path;

/// Scheduled-task name.
pub const TASK_NAME: &str = "fakescan-daemon";

/// The `/TR` command line the task runs: quoted exe, `--manage`, and the options the daemon was
/// installed with (the task starts with an unrelated working directory, so paths are absolute).
pub fn task_command(
    exe: &Path,
    profiles: &Path,
    listen: Option<&str>,
    advertise: Option<&str>,
) -> String {
    let mut c = format!(
        "\"{}\" --manage --profiles \"{}\"",
        exe.display(),
        profiles.display()
    );
    if let Some(l) = listen {
        c.push_str(&format!(" --listen {l}"));
    }
    if let Some(a) = advertise {
        c.push_str(&format!(" --advertise {a}"));
    }
    c
}

/// `schtasks` arguments that create the logon task for the current user (limited, no elevation
/// requested by the task itself).
pub fn create_args(command: &str) -> Vec<String> {
    [
        "/Create", "/TN", TASK_NAME, "/SC", "ONLOGON", "/RL", "LIMITED", "/F", "/TR",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain(std::iter::once(command.to_string()))
    .collect()
}

#[cfg(windows)]
fn schtasks(args: &[String]) -> anyhow::Result<(bool, String)> {
    let out = std::process::Command::new("schtasks.exe")
        .args(args)
        .output()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok((out.status.success(), text.trim().to_string()))
}

#[cfg(windows)]
pub fn install(
    profiles: &Path,
    listen: Option<&str>,
    advertise: Option<&str>,
) -> anyhow::Result<String> {
    let exe = std::env::current_exe()?;
    let profiles = profiles.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "--profiles {} must exist (the task runs elsewhere): {e}",
            profiles.display()
        )
    })?;
    // canonicalize yields a \\?\ path on Windows; the task command line wants a plain one.
    let profiles = std::path::PathBuf::from(profiles.to_string_lossy().trim_start_matches(r"\\?\"));
    let cmd = task_command(&exe, &profiles, listen, advertise);
    let (ok, text) = schtasks(&create_args(&cmd))?;
    anyhow::ensure!(ok, "schtasks /Create failed: {text}");
    Ok(format!("installed logon task {TASK_NAME}: {cmd}"))
}

#[cfg(windows)]
pub fn uninstall() -> anyhow::Result<String> {
    let args = ["/Delete", "/TN", TASK_NAME, "/F"].map(String::from);
    let (ok, text) = schtasks(&args)?;
    anyhow::ensure!(ok, "schtasks /Delete failed: {text}");
    Ok(format!("removed logon task {TASK_NAME}"))
}

#[cfg(windows)]
pub fn status() -> anyhow::Result<String> {
    let args = ["/Query", "/TN", TASK_NAME, "/FO", "LIST", "/V"].map(String::from);
    let (ok, text) = schtasks(&args)?;
    if ok {
        Ok(text)
    } else {
        Ok(format!("logon task {TASK_NAME} is not installed"))
    }
}

#[cfg(not(windows))]
pub fn install(_: &Path, _: Option<&str>, _: Option<&str>) -> anyhow::Result<String> {
    anyhow::bail!("daemon install: Windows only (use your init system to run `fakescan --manage`)")
}

#[cfg(not(windows))]
pub fn uninstall() -> anyhow::Result<String> {
    anyhow::bail!("daemon uninstall: Windows only")
}

#[cfg(not(windows))]
pub fn status() -> anyhow::Result<String> {
    anyhow::bail!("daemon status: Windows only")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_is_absolute_and_manages() {
        let c = task_command(
            Path::new(r"C:\x\fakescan.exe"),
            Path::new(r"C:\x\profiles"),
            Some("0.0.0.0:3920"),
            Some("192.168.1.70"),
        );
        assert_eq!(
            c,
            r#""C:\x\fakescan.exe" --manage --profiles "C:\x\profiles" --listen 0.0.0.0:3920 --advertise 192.168.1.70"#
        );
        let a = create_args(&c);
        assert!(a.contains(&"ONLOGON".to_string()) && a.contains(&"LIMITED".to_string()));
        assert_eq!(a.last().unwrap(), &c);
    }
}
