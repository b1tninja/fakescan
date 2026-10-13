//! Windows system-tray icon inside the daemon (cargo feature `tray`).
//!
//! The tray runs on its own thread with its own Win32 message loop and never takes the daemon
//! down: creation errors and panics are logged and the daemon carries on headless.
//!
//! * `tray-icon` draws the icon, tooltip and menu.
//! * The message pump (and a cross-thread wake-up `Notice`) comes from `native-windows-gui`'s
//!   safe API, because the workspace forbids `unsafe`.
//! * Device state is read by polling the daemon's own loopback `GET /v1/devices` every ~2 s from a
//!   helper thread (`DeviceManager` is not reachable from `main.rs`; see the report for the
//!   small `server.rs` change that would allow reading it in-process).
//! * Notifications use a hidden, detached `powershell.exe` NotifyIcon balloon (the approach of
//!   `Invoke-LabAttention` in `scripts/Fakescan.psm1`).

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use base64::Engine as _;
use native_windows_gui as nwg;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::tray_model::{Level, Summary, TransitionDetector};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const POLL_EVERY: Duration = Duration::from_secs(2);

const ID_OPEN_UI: &str = "open_ui";
const ID_ADD_SETTINGS: &str = "open_add_settings";
const ID_QUIT_YES: &str = "quit_yes";

/// What the tray needs from the daemon.
pub struct TrayConfig {
    /// The daemon's actual listen port (loopback is used to reach it).
    pub port: u16,
    /// Called once when the user confirms Quit; must trigger the daemon's graceful shutdown.
    pub quit: Arc<dyn Fn() + Send + Sync>,
}

/// Handle to a running tray thread.
pub struct TrayHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl TrayHandle {
    /// Remove the icon and join the tray thread (bounded wait). Safe to call more than once.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !t.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }
}

impl Drop for TrayHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start the tray on its own thread. Returns `None` (after logging) if the thread cannot be spawned;
/// later failures are logged from the thread and leave the daemon running headless.
pub fn spawn(cfg: TrayConfig) -> Option<TrayHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("fakescan-tray".into())
        .spawn(move || {
            tracing::info!("tray thread started");
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(cfg, stop2)));
            match r {
                Ok(Ok(())) => tracing::info!("tray thread ended"),
                Ok(Err(e)) => tracing::warn!(error = %e, "tray unavailable; continuing headless"),
                Err(_) => tracing::warn!("tray thread panicked; continuing headless"),
            }
        });
    match thread {
        Ok(t) => Some(TrayHandle {
            stop,
            thread: Some(t),
        }),
        Err(e) => {
            tracing::warn!(error = %e, "could not start tray thread; continuing headless");
            None
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Icon pixels
// ---------------------------------------------------------------------------------------------

pub const ICON_SIZE: u32 = 32;

fn level_color(level: Level) -> [u8; 3] {
    match level {
        Level::Green => [46, 160, 67],
        Level::Amber => [232, 163, 23],
        Level::Red => [214, 48, 49],
    }
}

fn fill(px: &mut [u8], x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 4]) {
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * ICON_SIZE + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&c);
        }
    }
}

/// RGBA pixels of a flatbed-scanner glyph in the colour of `level` (32x32, transparent corners).
pub fn icon_rgba(level: Level) -> Vec<u8> {
    let [r, g, b] = level_color(level);
    let mut px = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    let dark = [30, 30, 36, 255];
    // Lid (outline, light fill).
    fill(&mut px, 3, 5, 29, 15, dark);
    fill(&mut px, 4, 6, 28, 14, [236, 238, 242, 255]);
    // Body (outline, coloured fill).
    fill(&mut px, 1, 14, 31, 27, dark);
    fill(&mut px, 2, 15, 30, 26, [r, g, b, 255]);
    // Glass strip with a scan line.
    fill(&mut px, 5, 17, 27, 22, [250, 250, 252, 255]);
    fill(&mut px, 5, 19, 27, 20, [r / 2, g / 2, b / 2, 255]);
    // Status LED.
    fill(&mut px, 25, 23, 28, 25, [255, 255, 255, 255]);
    px
}

fn make_icon(level: Level) -> Result<Icon, tray_icon::BadIcon> {
    Icon::from_rgba(icon_rgba(level), ICON_SIZE, ICON_SIZE)
}

// ---------------------------------------------------------------------------------------------
// Notification helper (hidden PowerShell NotifyIcon balloon)
// ---------------------------------------------------------------------------------------------

const HELPER_TEMPLATE: &str = r#"Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$script:clicked = $false
$ni = New-Object System.Windows.Forms.NotifyIcon
$ni.Icon = [System.Drawing.SystemIcons]::Warning
$ni.Text = 'fakescan: Add device required (click to open Settings)'
$ni.Visible = $true
# Windows 11 raises a spurious click shortly after the balloon appears; ignore the first 2.5 s.
$script:shownAt = [datetime]::UtcNow
$open = { if (([datetime]::UtcNow - $script:shownAt).TotalSeconds -lt 2.5) { return }; Start-Process 'ms-settings:printers'; $script:clicked = $true }
$ni.add_BalloonTipClicked($open)
$ni.add_Click($open)
$script:shownAt = [datetime]::UtcNow
$ni.ShowBalloonTip(15000, 'fakescan - Add device required', '__DETAIL__ Click to open Settings.', [System.Windows.Forms.ToolTipIcon]::Warning)
$end = (Get-Date).AddSeconds(__HOLD__)
while (-not $script:clicked -and (Get-Date) -lt $end) {
    [System.Windows.Forms.Application]::DoEvents()
    Start-Sleep -Milliseconds 100
}
Start-Sleep -Milliseconds 300
$ni.Visible = $false
$ni.Dispose()
"#;

/// Make `s` safe inside a PowerShell single-quoted string: every quote-like character (PowerShell
/// also treats the typographic single quotes as quotes) is doubled, control characters become
/// spaces, and the length is capped (balloon bodies are limited to ~255 characters).
pub fn ps_single_quoted(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars().take(200) {
        match ch {
            '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => {
                out.push('\'');
                out.push('\'');
            }
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// The helper script text for `detail`, held alive `hold_secs` (min 5).
pub fn notify_script(detail: &str, hold_secs: u32) -> String {
    HELPER_TEMPLATE
        .replace("__DETAIL__", &ps_single_quoted(detail))
        .replace("__HOLD__", &hold_secs.max(5).to_string())
}

/// Base64 of the UTF-16LE script, the form `powershell -EncodedCommand` expects.
pub fn encode_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn hidden(cmd: &mut Command) -> &mut Command {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
}

fn spawn_notification(detail: &str) {
    let enc = encode_command(&notify_script(detail, 120));
    let mut cmd = Command::new("powershell.exe");
    hidden(&mut cmd).args([
        "-NoProfile",
        "-STA",
        "-ExecutionPolicy",
        "Bypass",
        "-EncodedCommand",
        &enc,
    ]);
    match cmd.spawn() {
        Ok(mut child) => {
            // Reap it when it exits (up to ~2 minutes) without blocking anything.
            std::thread::spawn(move || {
                let r = child.wait();
                tracing::debug!(status = ?r, "tray notification helper exited");
            });
        }
        Err(e) => tracing::warn!(error = %e, "tray notification helper failed to start"),
    }
}

/// `cmd /C start "" TARGET` (default browser or protocol handler). `target` is built by us only.
fn shell_open(target: &str) {
    let mut cmd = Command::new("cmd");
    hidden(&mut cmd).args(["/C", "start", "", target]);
    if let Err(e) = cmd.spawn().map(|mut c| {
        std::thread::spawn(move || {
            let _ = c.wait();
        });
    }) {
        tracing::warn!(error = %e, target, "tray: could not open");
    }
}

// ---------------------------------------------------------------------------------------------
// Polling the daemon
// ---------------------------------------------------------------------------------------------

fn fetch_summary(port: u16) -> Summary {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let go = || -> std::io::Result<Vec<u8>> {
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(1))?;
        s.set_read_timeout(Some(Duration::from_secs(2)))?;
        s.set_write_timeout(Some(Duration::from_secs(2)))?;
        write!(
            s,
            "GET /v1/devices HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
        )?;
        let mut buf = Vec::new();
        s.read_to_end(&mut buf)?;
        Ok(buf)
    };
    match go() {
        Ok(buf) => crate::tray_model::summary_from_http_response(&buf),
        Err(e) => Summary::error(e.to_string()),
    }
}

type Shared = Arc<Mutex<Option<Summary>>>;

fn poller(port: u16, stop: Arc<AtomicBool>, shared: Shared, wake: nwg::NoticeSender) {
    let t0 = Instant::now();
    let mut detector = TransitionDetector::new();
    let mut last: Option<Summary> = None;
    while !stop.load(Ordering::SeqCst) {
        let s = fetch_summary(port);
        if let Some(n) = detector.observe(&s, t0.elapsed()) {
            tracing::info!(devices = ?n.devices, startup = n.startup, "tray: notifying");
            spawn_notification(&n.detail);
        }
        if last.as_ref() != Some(&s) {
            if let Ok(mut g) = shared.lock() {
                *g = Some(s.clone());
            }
            last = Some(s);
        }
        // Always wake the pump so it can see `stop`; cheap.
        wake.notice();
        let until = Instant::now() + POLL_EVERY;
        while Instant::now() < until && !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    wake.notice();
}

// ---------------------------------------------------------------------------------------------
// Tray thread body
// ---------------------------------------------------------------------------------------------

fn build_menu(s: &Summary) -> Result<Menu, tray_icon::menu::Error> {
    let menu = Menu::new();
    menu.append(&MenuItem::with_id(
        ID_OPEN_UI,
        "Open device manager",
        true,
        None,
    ))?;
    menu.append(&PredefinedMenuItem::separator())?;
    if let Some(e) = &s.error {
        menu.append(&MenuItem::new(
            format!("daemon not reachable: {e}"),
            false,
            None,
        ))?;
    } else if s.devices.is_empty() {
        menu.append(&MenuItem::new("no devices", false, None))?;
    }
    for d in &s.devices {
        menu.append(&MenuItem::new(&d.text, false, None))?;
    }
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&MenuItem::with_id(
        ID_ADD_SETTINGS,
        "Open Add device settings",
        true,
        None,
    ))?;
    let yes = MenuItem::with_id(
        ID_QUIT_YES,
        "Yes, stop all emulated devices and quit",
        true,
        None,
    );
    menu.append(&Submenu::with_items("Quit fakescan", true, &[&yes])?)?;
    Ok(menu)
}

fn apply(tray: &TrayIcon, s: &Summary) {
    match make_icon(s.level) {
        Ok(i) => {
            let _ = tray.set_icon(Some(i));
        }
        Err(e) => tracing::warn!(error = %e, "tray icon"),
    }
    let _ = tray.set_tooltip(Some(s.tooltip()));
    match build_menu(s) {
        Ok(m) => tray.set_menu(Some(Box::new(m))),
        Err(e) => tracing::warn!(error = %e, "tray menu"),
    }
}

fn run(cfg: TrayConfig, stop: Arc<AtomicBool>) -> anyhow::Result<()> {
    nwg::init().map_err(|e| anyhow::anyhow!("nwg init: {e:?}"))?;
    let mut window = nwg::MessageWindow::default();
    nwg::MessageWindow::builder()
        .build(&mut window)
        .map_err(|e| anyhow::anyhow!("message window: {e:?}"))?;
    let mut notice = nwg::Notice::default();
    nwg::Notice::builder()
        .parent(&window)
        .build(&mut notice)
        .map_err(|e| anyhow::anyhow!("notice: {e:?}"))?;

    let initial = Summary::error("starting");
    let tray = TrayIconBuilder::new()
        .with_icon(make_icon(initial.level)?)
        .with_tooltip("fakescan: starting")
        .with_menu(Box::new(build_menu(&initial)?))
        .build()?;

    let port = cfg.port;
    let quit = Arc::clone(&cfg.quit);
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| match e.id.0.as_str() {
        ID_OPEN_UI => shell_open(&format!("http://127.0.0.1:{port}/ui")),
        ID_ADD_SETTINGS => shell_open("ms-settings:printers"),
        ID_QUIT_YES => {
            tracing::info!("tray: quit confirmed");
            quit();
            nwg::stop_thread_dispatch();
        }
        _ => {}
    }));

    let shared: Shared = Arc::new(Mutex::new(None));
    let poll = {
        let (stop, shared, wake) = (Arc::clone(&stop), Arc::clone(&shared), notice.sender());
        std::thread::Builder::new()
            .name("fakescan-tray-poll".into())
            .spawn(move || poller(port, stop, shared, wake))?
    };

    let notice_handle = notice.handle;
    let stop2 = Arc::clone(&stop);
    let handler = nwg::full_bind_event_handler(&window.handle, move |evt, _data, handle| {
        if evt == nwg::Event::OnNotice && handle == notice_handle {
            if stop2.load(Ordering::SeqCst) {
                nwg::stop_thread_dispatch();
                return;
            }
            let next = shared.lock().ok().and_then(|mut g| g.take());
            if let Some(s) = next {
                apply(&tray, &s);
            }
        }
    });
    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
    stop.store(true, Ordering::SeqCst);
    let _ = poll.join();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn icon_buffers() {
        let mut seen = HashSet::new();
        for lvl in [Level::Green, Level::Amber, Level::Red] {
            let px = icon_rgba(lvl);
            assert_eq!(px.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
            assert!(px.chunks(4).any(|p| p[3] == 255), "opaque pixels");
            assert!(px.chunks(4).any(|p| p[3] == 0), "transparent corners");
            // Body colour pixel (row 16, col 3) carries the level colour.
            let i = ((16 * ICON_SIZE + 3) * 4) as usize;
            assert_eq!(&px[i..i + 3], &level_color(lvl));
            seen.insert(px);
            assert!(make_icon(lvl).is_ok());
        }
        assert_eq!(seen.len(), 3);
    }

    fn decode(enc: &str) -> String {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(enc)
            .unwrap();
        let u: Vec<u16> = bytes
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&u).unwrap()
    }

    #[test]
    fn encoded_command_round_trips() {
        let script = notify_script("1 device needs an Add in Windows: tray5.", 120);
        assert_eq!(decode(&encode_command(&script)), script);
        assert!(script.contains("ms-settings:printers"));
        assert!(script.contains("1 device needs an Add in Windows: tray5. Click to open Settings."));
        assert!(script.contains("AddSeconds(120)"));
        assert!(!script.contains("__"));
    }

    #[test]
    fn detail_is_injection_safe() {
        let evil = "x'); Remove-Item C:\\ -Recurse; ('\u{2019}\r\nnext";
        let q = ps_single_quoted(evil);
        // Every quote-like char is doubled and there is no raw newline.
        assert!(!q.contains('\n') && !q.contains('\r') && !q.contains('\u{2019}'));
        // Removing doubled quote pairs leaves no lone quote: the string cannot terminate early.
        assert_eq!(q.replace("''", "").matches('\'').count(), 0);
        let script = notify_script(evil, 5);
        assert!(script.contains(&format!("'{q} Click to open Settings.'")));
        // Length cap.
        assert!(ps_single_quoted(&"a".repeat(1000)).len() <= 200);
    }
}
