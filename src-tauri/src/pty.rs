use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

pub struct PtyState {
    sessions: Mutex<HashMap<String, Session>>,
}

struct Session {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

impl Default for PtyState {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}

fn require_id(id: &str) -> Result<String, String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("Falta el id de la terminal".to_string());
    }
    Ok(trimmed.to_string())
}

fn resolve_cwd(cwd: &str, home: PathBuf) -> String {
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        home.to_string_lossy().into_owned()
    } else {
        trimmed.to_string()
    }
}

fn kill_session(sessions: &mut HashMap<String, Session>, id: &str) {
    if let Some(mut current) = sessions.remove(id) {
        let _ = current.killer.kill();
    }
}

#[tauri::command]
pub fn pty_spawn(
    app: AppHandle,
    state: State<PtyState>,
    id: String,
    cwd: String,
    cols: u16,
    rows: u16,
    on_data: Channel<String>,
    on_exit: Channel<i32>,
) -> Result<(), String> {
    let id = require_id(&id)?;
    let cwd = resolve_cwd(
        &cwd,
        app.path()
            .home_dir()
            .map_err(|error| format!("No se pudo resolver el home: {error}"))?,
    );

    let mut sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    kill_session(&mut sessions, &id);

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("No se pudo abrir la terminal: {error}"))?;

    let mut command = CommandBuilder::new(default_shell());
    command.cwd(&cwd);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");

    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| format!("No se pudo lanzar el shell: {error}"))?;

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| error.to_string())?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| error.to_string())?;
    let killer = child.clone_killer();

    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let chunk = String::from_utf8_lossy(&buffer[..count]).into_owned();
                    if on_data.send(chunk).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    std::thread::spawn(move || {
        let code = match child.wait() {
            Ok(status) => i32::try_from(status.exit_code()).unwrap_or(1),
            Err(_) => 1,
        };
        let _ = on_exit.send(code);
    });

    sessions.insert(
        id,
        Session {
            writer,
            master: pair.master,
            killer,
        },
    );

    Ok(())
}

#[tauri::command]
pub fn pty_write(state: State<PtyState>, id: String, data: String) -> Result<(), String> {
    let id = require_id(&id)?;
    let mut sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| "No hay terminal".to_string())?;

    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|error| error.to_string())?;
    session.writer.flush().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn pty_resize(state: State<PtyState>, id: String, cols: u16, rows: u16) -> Result<(), String> {
    let id = require_id(&id)?;
    let sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    let session = sessions
        .get(&id)
        .ok_or_else(|| "No hay terminal".to_string())?;

    session
        .master
        .resize(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn pty_kill(state: State<PtyState>, id: String) -> Result<(), String> {
    let id = require_id(&id)?;
    let mut sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    kill_session(&mut sessions, &id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_id_rejects_blank() {
        assert!(require_id("").is_err());
        assert!(require_id("   ").is_err());
        assert_eq!(require_id(" preview ").unwrap(), "preview");
    }

    #[test]
    fn empty_cwd_uses_home() {
        let home = PathBuf::from("/home/fernando");
        assert_eq!(resolve_cwd("", home.clone()), "/home/fernando");
        assert_eq!(resolve_cwd("  ", home.clone()), "/home/fernando");
        assert_eq!(resolve_cwd("/tmp/notes", home), "/tmp/notes");
    }
}
