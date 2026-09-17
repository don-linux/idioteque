//! Host ↔ ADE line protocol (CONTRACT.md §4.3 / §4.4).
//! All protocol JSON is written to the duplicated original stdout fd.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::sync::{Mutex, OnceLock};
use std::thread;

use serde::{Deserialize, Serialize};

static WRITER: OnceLock<Mutex<File>> = OnceLock::new();

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum HostEvent {
    Ready {
        cef: String,
        chromium: String,
        #[serde(rename = "apiVersion")]
        api_version: u32,
        xid: u64,
    },
    Nav {
        url: String,
        #[serde(rename = "canGoBack")]
        can_go_back: bool,
        #[serde(rename = "canGoForward")]
        can_go_forward: bool,
        loading: bool,
    },
    Title {
        title: String,
    },
    LoadEnd {
        status: i32,
    },
    LoadError {
        code: i32,
        text: String,
        url: String,
    },
    Shortcut {
        chord: String,
    },
    RenderCrashed {
        status: String,
    },
    Health {
        ok: bool,
        cef: String,
        chromium: String,
        #[serde(rename = "apiVersion")]
        api_version: u32,
    },
    Fatal {
        message: String,
        code: i32,
    },
    Info {
        #[serde(rename = "apiVersion")]
        api_version: u32,
        #[serde(rename = "cefCompiled")]
        cef_compiled: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum HostCommand {
    Navigate {
        url: String,
    },
    Back,
    Forward,
    Stop,
    Reload {
        #[serde(rename = "ignoreCache", default)]
        ignore_cache: bool,
    },
    SetBounds {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    Show,
    Hide,
    Focus,
    Devtools,
    Close,
}

/// Take ownership of the duplicated protocol fd. Must be called once, at startup.
pub fn init_from_raw_fd(fd: RawFd) {
    let file = unsafe { File::from_raw_fd(fd) };
    let _ = WRITER.set(Mutex::new(file));
}

pub fn emit(event: &HostEvent) {
    let Some(lock) = WRITER.get() else {
        return;
    };
    let Ok(mut file) = lock.lock() else {
        return;
    };
    if let Ok(buf) = serde_json::to_vec(event) {
        let _ = file.write_all(&buf);
        let _ = file.write_all(b"\n");
        let _ = file.flush();
    }
}

/// Background stdin reader. Lines are posted onto the CEF UI thread. EOF → Close.
pub fn spawn_stdin_reader() {
    thread::Builder::new()
        .name("idq-stdin".into())
        .spawn(|| {
            let stdin = io::stdin();
            let reader = BufReader::new(stdin.lock());
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<HostCommand>(line) {
                            Ok(cmd) => crate::app::post_cmd(cmd),
                            Err(_) => {
                                // Unknown / malformed: ignore (CONTRACT: unknown args ignored).
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            crate::app::post_cmd(HostCommand::Close);
        })
        .expect("spawn stdin reader");
}
