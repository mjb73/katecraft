//! One window per user, always the current build.
//!
//! Kate opens a file in the window that is already running instead of piling
//! up copies, and KateCraft does the same here: a second launch hands its
//! files to the open window and exits. The handshake carries the identity of
//! the running program (its size and build time), so a window left over from
//! an older build is recognised and replaced rather than reused — opening a
//! file from the file manager can never bring an old editor back to the front.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

/// First line of the little protocol spoken over the socket.
const MAGIC: &str = "katecraft-ipc";
const VERSION: &str = "1";

/// What happened to a second launch.
pub enum Outcome {
    /// A window was already running; the files went there.
    Forwarded,
    /// Nothing was running (or only a dead socket was left behind).
    Absent,
    /// A window from another build answered; it may still be closing.
    OlderBuild,
}

/// The listening end owned by the window that is running.
pub struct Server {
    rx: Receiver<Vec<PathBuf>>,
    replace: Arc<AtomicBool>,
}

impl Server {
    /// Files sent by later launches, whether a launch asked for the window at
    /// all, and whether a newer build asked this window to step aside.
    pub fn poll(&mut self) -> (Vec<PathBuf>, bool, bool) {
        let mut files = Vec::new();
        let mut heard = false;
        while let Ok(mut more) = self.rx.try_recv() {
            heard = true;
            files.append(&mut more);
        }
        let replace = self.replace.swap(false, Ordering::SeqCst);
        (files, heard, replace)
    }
}

// ---------------------------------------------------------------- launcher ---

/// Send this launch's files to the editor that is already running. A socket
/// left behind by a window that has closed refuses connections and is cleared;
/// a socket owned by another build answers `OLD` and that build is not stopped
/// here, in case it is holding unsaved work.
pub fn forward(files: &[PathBuf]) -> Outcome {
    #[cfg(not(unix))]
    {
        let _ = files;
        return Outcome::Absent;
    }

    #[cfg(unix)]
    {
        let path = socket_path();
        let mut retried = false;
        loop {
            match UnixStream::connect(&path) {
                Ok(mut stream) => {
                    return match handshake(&mut stream, files) {
                        Ok(Reply::Ok) => Outcome::Forwarded,
                        Ok(Reply::Old(pid)) => {
                            // Another build owns the socket. Give it a moment
                            // to close; if it is holding unsaved work it stays
                            // open and this launch becomes a separate window.
                            wait_gone(pid, Duration::from_millis(1500));
                            Outcome::OlderBuild
                        }
                        _ => Outcome::OlderBuild,
                    };
                }
                Err(e)
                    if e.kind() == io::ErrorKind::NotFound
                        || e.kind() == io::ErrorKind::ConnectionRefused =>
                {
                    if !retried {
                        // A window may be binding its socket this instant.
                        retried = true;
                        std::thread::sleep(Duration::from_millis(180));
                        continue;
                    }
                    let _ = std::fs::remove_file(&path);
                    return Outcome::Absent;
                }
                Err(_) => return Outcome::Absent,
            }
        }
    }
}

/// Become the one window for this user and listen for later launches.
/// `clear_old` stops a window left by a build that has no handshake; it is
/// false when that window answered and may still be closing with work in it.
pub fn claim(clear_old: bool) -> Server {
    #[cfg(not(unix))]
    {
        let _ = clear_old;
        let (_tx, rx) = channel();
        return Server {
            rx,
            replace: Arc::new(AtomicBool::new(false)),
        };
    }

    #[cfg(unix)]
    {
        if clear_old {
            stop_stale();
        }
        let path = socket_path();
        let (tx, rx) = channel();
        let replace = Arc::new(AtomicBool::new(false));

        for attempt in 0..20 {
            match UnixListener::bind(&path) {
                Ok(listener) => {
                    spawn_server(listener, tx, replace.clone());
                    return Server { rx, replace };
                }
                Err(_) => {
                    // Either the file is dead or another copy won the race.
                    if let Ok(mut stream) = UnixStream::connect(&path) {
                        if let Ok(Reply::Ok) = handshake(&mut stream, &[]) {
                            // It is alive after all; keep listening nowhere and
                            // let this window open (it simply will not receive).
                            return Server { rx, replace };
                        }
                    }
                    if attempt == 0 {
                        std::thread::sleep(Duration::from_millis(120));
                    }
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        Server { rx, replace }
    }
}

// ---------------------------------------------------------------- protocol ---

enum Reply {
    Ok,
    Old(u32),
    Bad,
}

#[cfg(unix)]
fn handshake(stream: &mut UnixStream, files: &[PathBuf]) -> io::Result<Reply> {
    stream.set_write_timeout(Some(Duration::from_millis(1200)))?;
    stream.set_read_timeout(Some(Duration::from_millis(1200)))?;

    let mut msg = Vec::new();
    msg.extend_from_slice(format!("{MAGIC} {VERSION} {} {}\n", build_id(), files.len()).as_bytes());
    for f in files {
        let bytes = f.as_os_str().as_bytes();
        msg.extend_from_slice(format!("{}\n", bytes.len()).as_bytes());
        msg.extend_from_slice(bytes);
    }
    stream.write_all(&msg)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let line = line.trim_end();
    if line == "OK" {
        Ok(Reply::Ok)
    } else if let Some(pid) = line.strip_prefix("OLD ") {
        Ok(pid
            .trim()
            .parse()
            .map(Reply::Old)
            .unwrap_or(Reply::Bad))
    } else {
        Ok(Reply::Bad)
    }
}

#[cfg(unix)]
fn spawn_server(listener: UnixListener, tx: Sender<Vec<PathBuf>>, replace: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_millis(1500)));
            let Ok(read_half) = stream.try_clone() else { continue };
            let mut reader = BufReader::new(read_half);

            let mut header = String::new();
            if reader.read_line(&mut header).is_err() {
                continue;
            }
            let fields: Vec<&str> = header.trim_end().split(' ').collect();
            if fields.len() != 4 || fields[0] != MAGIC || fields[1] != VERSION {
                let _ = stream.write_all(b"BAD\n");
                continue;
            }
            let count: usize = fields[3].parse().unwrap_or(0);

            // Read the whole message first, so a reply is never lost to a
            // reset while unread bytes are still in flight.
            let mut files = Vec::new();
            let mut complete = true;
            for _ in 0..count {
                let mut len_line = String::new();
                if reader.read_line(&mut len_line).is_err() {
                    complete = false;
                    break;
                }
                let len: usize = match len_line.trim().parse() {
                    Ok(n) if n <= 1 << 20 => n,
                    _ => {
                        complete = false;
                        break;
                    }
                };
                let mut buf = vec![0u8; len];
                if reader.read_exact(&mut buf).is_err() {
                    complete = false;
                    break;
                }
                files.push(PathBuf::from(std::ffi::OsString::from_vec(buf)));
            }
            if !complete {
                let _ = stream.write_all(b"BAD\n");
                continue;
            }
            if fields[2] != build_id() {
                // Somebody built a newer copy and launched it.
                let _ = stream.write_all(format!("OLD {}\n", std::process::id()).as_bytes());
                replace.store(true, Ordering::SeqCst);
                continue;
            }

            let _ = tx.send(files);
            let _ = stream.write_all(b"OK\n");
        }
    });
}

// ------------------------------------------------------------- housekeeping ---

/// A stable per-user name next to the other session sockets.
fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    dir.join(format!("katecraft-{user}.sock"))
}

/// Identifies the program that is running. It is a fingerprint of the whole
/// executable, so two copies of one build (the launcher copy and the freshly
/// built one) agree, while any rebuild gets a different one.
fn build_id() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        let Ok(path) = std::env::current_exe() else {
            return "0".to_string();
        };
        let Ok(data) = std::fs::read(path) else {
            return "0".to_string();
        };
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in &data {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{hash:016x}-{:x}", data.len())
    })
}

/// The command line of a KateCraft process, used to leave picture-taking runs
/// (which never own a window on screen) alone.
#[cfg(unix)]
fn is_picture_run(pid: u32, dir: &std::path::Path) -> bool {
    let cmd = std::fs::read(dir.join(pid.to_string()).join("cmdline")).unwrap_or_default();
    cmd.split(|b| *b == 0).any(|arg| arg == b"--screenshot")
}

/// Windows left by builds from before this handshake existed. They cannot be
/// asked to close, so they are the only processes ever stopped here.
#[cfg(unix)]
fn stop_stale() {
    let me = std::process::id();
    let mut pids = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            if pid == me {
                continue;
            }
            let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
            if comm.trim() != "katecraft" {
                continue;
            }
            if is_picture_run(pid, &entry.path()) {
                continue;
            }
            pids.push(pid);
        }
    }
    if pids.is_empty() {
        return;
    }
    for pid in &pids {
        let _ = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
    }
    let deadline = std::time::Instant::now() + Duration::from_millis(2000);
    while std::time::Instant::now() < deadline {
        if pids.iter().all(|p| !std::path::Path::new(&format!("/proc/{p}")).exists()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    for pid in pids {
        if std::path::Path::new(&format!("/proc/{pid}")).exists() {
            let _ = std::process::Command::new("kill")
                .arg("-KILL")
                .arg(pid.to_string())
                .status();
        }
    }
}

fn wait_gone(pid: u32, limit: Duration) {
    let deadline = std::time::Instant::now() + limit;
    while std::time::Instant::now() < deadline {
        if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(60));
    }
}

// ------------------------------------------------------------ installation ---

/// The launcher starts `~/.local/bin/katecraft`, a copy of the built program.
/// When a newer build exists in the project, replace the copy and restart from
/// it, so the launcher (and the files opened from the file manager) always run
/// the build that was just made instead of yesterday's.
pub fn refresh_installed_copy() {
    if std::env::var_os("KATECRAFT_NO_SELF_UPDATE").is_some() {
        return;
    }
    let Ok(current) = std::env::current_exe() else { return };
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return };
    let installed = home.join(".local/bin/katecraft");

    let same = match (std::fs::canonicalize(&current), std::fs::canonicalize(&installed)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same {
        return;
    }

    let built = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/katecraft");
    let (Ok(old), Ok(new)) = (std::fs::metadata(&current), std::fs::metadata(&built)) else {
        return;
    };
    let newer = match (old.modified(), new.modified()) {
        (Ok(a), Ok(b)) => b > a,
        _ => false,
    };
    if !newer || old.len() == new.len() && old.modified().ok() == new.modified().ok() {
        return;
    }

    let staging = installed.with_file_name("katecraft.new");
    if std::fs::copy(&built, &staging).is_err() {
        let _ = std::fs::remove_file(&staging);
        return;
    }
    let _ = std::fs::set_permissions(&staging, new.permissions());
    if std::fs::rename(&staging, &installed).is_err() {
        let _ = std::fs::remove_file(&staging);
        return;
    }
    // Start the fresh copy with the same files, then step aside.
    if std::process::Command::new(&installed)
        .args(std::env::args_os().skip(1))
        .spawn()
        .is_ok()
    {
        std::process::exit(0);
    }
}
