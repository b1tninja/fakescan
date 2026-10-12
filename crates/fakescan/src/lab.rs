//! Stable lab identity: one TCP port, one device UUID, one process.
//!
//! Windows PnP / WIA association keys off advertise URL + UUID. Restarting with a
//! different listen/uuid forces remove+re-Add. Prefer `/v1/*` reconfigure and
//! `scripts/fakescan.ps1 ensure` over kill/restart.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::process;

use uuid::Uuid;

/// Canonical lab TCP port (never change without a new UUID).
pub const LAB_PORT: u16 = 3920;

/// Canonical WS-Discovery / PnP device UUID for the Windows lab host.
pub const LAB_DEVICE_UUID: &str = "00000000-0000-4000-8000-0000000000f2";

pub fn lab_device_uuid() -> Uuid {
    Uuid::parse_str(LAB_DEVICE_UUID).expect("LAB_DEVICE_UUID")
}

pub fn default_listen() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::UNSPECIFIED, LAB_PORT))
}

/// First non-loopback IPv4 (for `--advertise` when listen is `0.0.0.0`).
pub fn first_lan_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    // No packets sent; OS picks the interface for this route.
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

fn lock_path(port: u16) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("fakescan-{port}.lock"));
    dir
}

pub struct InstanceLock {
    _file: File,
    path: PathBuf,
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Exclusive lab lock for `port`. Stale locks (dead PID) are replaced.
pub fn acquire_instance_lock(port: u16) -> anyhow::Result<InstanceLock> {
    let path = lock_path(port);
    match try_create_lock(&path, port) {
        Ok(file) => Ok(InstanceLock { _file: file, path }),
        Err(_) => {
            if let Some(stale) = read_lock_pid(&path) {
                if !pid_alive(stale) {
                    let _ = fs::remove_file(&path);
                    let file = try_create_lock(&path, port)?;
                    return Ok(InstanceLock { _file: file, path });
                }
            }
            Err(anyhow::anyhow!(already_running_message(port)))
        }
    }
}

fn try_create_lock(path: &std::path::Path, port: u16) -> std::io::Result<File> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    writeln!(
        f,
        "pid={}\nport={}\nuuid={}\n",
        process::id(),
        port,
        LAB_DEVICE_UUID
    )?;
    f.sync_all()?;
    Ok(f)
}

fn read_lock_pid(path: &std::path::Path) -> Option<u32> {
    let mut s = String::new();
    File::open(path).ok()?.read_to_string(&mut s).ok()?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("pid=") {
            return rest.trim().parse().ok();
        }
    }
    None
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    use std::os::windows::process::CommandExt;
    let out = process::Command::new("tasklist")
        .args(["/NH", "/FI", &format!("PID eq {pid}")])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()),
        Err(_) => true,
    }
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(any(windows, unix)))]
fn pid_alive(_pid: u32) -> bool {
    true
}

pub fn already_running_message(port: u16) -> String {
    format!(
        "fakescan already running on :{port} (lab UUID {LAB_DEVICE_UUID}).\n\
         Attach:  Invoke-RestMethod http://127.0.0.1:{port}/v1/status\n\
         Reconfigure via /v1/* — a kill skips discovery Bye and sticks WIA offline.\n\
         Replace this binary:  .\\scripts\\fakescan.ps1 stop\n\
         Start helper:  .\\scripts\\fakescan.ps1 ensure"
    )
}

/// Bind TCP; on AddrInUse print attach guidance.
pub async fn bind_listener(addr: SocketAddr) -> anyhow::Result<tokio::net::TcpListener> {
    match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => Ok(l),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            Err(anyhow::anyhow!(already_running_message(addr.port())))
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_uuid_parses() {
        assert_eq!(lab_device_uuid().to_string(), LAB_DEVICE_UUID);
    }

    #[test]
    fn default_listen_is_all_interfaces_3920() {
        assert_eq!(default_listen().port(), 3920);
        assert!(default_listen().ip().is_unspecified());
    }
}
