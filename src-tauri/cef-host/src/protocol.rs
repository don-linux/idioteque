//! Host ↔ ADE line protocol (CONTRACT.md §4.3 / §4.4).
//! All protocol JSON is written to the duplicated original stdout fd.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::sync::{Mutex, OnceLock};
use std::thread;

use serde::{Deserialize, Serialize};

static WRITER: OnceLock<Mutex<File>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    /// Printable text swallowed while chrome owns the keyboard (Ozone still
    /// delivers keys to the child; wry never sees them).
    Keys {
        text: String,
    },
    Focus {
        owner: FocusOwner,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next: Option<bool>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    Unfocus,
    /// Page click while chrome owns keys: `set_focus(true)`, no `XSetInputFocus`.
    Activate,
    Devtools,
    Close,
}

/// Who should own the keyboard after a `focus` event (CONTRACT: one owner).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FocusOwner {
    App,
    Browser,
}

/// Why the stdin pump stopped. All three map to orderly shutdown / exit 0
/// (CONTRACT §4.4, §4.7). None of them emit `fatal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StdinStop {
    Eof,
    ReadError,
    CloseCommand,
}

/// Take ownership of the duplicated protocol fd. Must be called once, at startup.
pub fn init_from_raw_fd(fd: RawFd) {
    let file = unsafe { File::from_raw_fd(fd) };
    let _ = WRITER.set(Mutex::new(file));
}

/// Write one JSON object plus a single trailing `\n`. Errors (including EPIPE
/// when the ADE is gone) are returned; `emit` swallows them so a broken pipe
/// cannot panic the host. The Rust runtime already ignores SIGPIPE.
fn write_event(file: &mut impl Write, event: &HostEvent) -> io::Result<()> {
    let mut buf =
        serde_json::to_vec(event).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    buf.push(b'\n');
    file.write_all(&buf)?;
    file.flush()
}

pub fn emit(event: &HostEvent) {
    let Some(lock) = WRITER.get() else {
        return;
    };
    let Ok(mut file) = lock.lock() else {
        return;
    };
    // Keep swallowing write errors: ADE gone → EPIPE. Do not unwrap.
    let _ = write_event(&mut *file, event);
}

/// CONTRACT §4.4 / §4.7: stdin EOF, a stdin read error, or `close` → exit 0.
#[cfg(test)]
fn exit_code_for_stdin_stop(stop: StdinStop) -> i32 {
    match stop {
        StdinStop::Eof | StdinStop::ReadError | StdinStop::CloseCommand => 0,
    }
}

fn parse_command_line(line: &str) -> Option<HostCommand> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    // Unknown / malformed: ignore (CONTRACT: unknown args ignored).
    serde_json::from_str(line).ok()
}

/// Read newline-terminated command lines. A final fragment without `\n` is
/// treated as a partial write (ADE died mid-line) and is not a command.
/// After `close`, further commands are discarded. EOF / read error without a
/// prior `close` posts `Close` so the host shuts down orderly (exit 0).
fn pump_stdin<R, F>(mut reader: R, mut post: F) -> StdinStop
where
    R: BufRead,
    F: FnMut(HostCommand),
{
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => {
                post(HostCommand::Close);
                return StdinStop::Eof;
            }
            Ok(_) => {
                if !buf.ends_with(&[b'\n']) {
                    // Partial last line: do not parse, do not execute.
                    post(HostCommand::Close);
                    return StdinStop::Eof;
                }
                match parse_command_line(std::str::from_utf8(&buf).unwrap_or("")) {
                    Some(HostCommand::Close) => {
                        post(HostCommand::Close);
                        return StdinStop::CloseCommand;
                    }
                    Some(cmd) => post(cmd),
                    None => {}
                }
            }
            Err(_) => {
                post(HostCommand::Close);
                return StdinStop::ReadError;
            }
        }
    }
}

/// Background stdin reader. Lines are posted onto the CEF UI thread. EOF → Close.
pub fn spawn_stdin_reader() {
    thread::Builder::new()
        .name("idq-stdin".into())
        .spawn(|| {
            let stdin = io::stdin();
            let reader = BufReader::new(stdin.lock());
            let _ = pump_stdin(reader, crate::app::post_cmd);
        })
        .expect("spawn stdin reader");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};
    use std::sync::{Arc, Mutex};

    fn posted(stdin: &str) -> (Vec<HostCommand>, StdinStop) {
        posted_bytes(stdin.as_bytes())
    }

    fn posted_bytes(stdin: &[u8]) -> (Vec<HostCommand>, StdinStop) {
        let mut out = Vec::new();
        let stop = pump_stdin(BufReader::new(stdin), |cmd| out.push(cmd));
        (out, stop)
    }

    fn posted_reader<R: Read>(reader: R) -> (Vec<HostCommand>, StdinStop) {
        let mut out = Vec::new();
        let stop = pump_stdin(BufReader::new(reader), |cmd| out.push(cmd));
        (out, stop)
    }

    struct OneByte<'a>(&'a [u8]);

    impl Read for OneByte<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.0.is_empty() || buf.is_empty() {
                return Ok(0);
            }
            buf[0] = self.0[0];
            self.0 = &self.0[1..];
            Ok(1)
        }
    }

    struct FailAfter {
        data: &'static [u8],
        pos: usize,
    }

    impl Read for FailAfter {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.pos >= self.data.len() {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "stdin died"));
            }
            let n = (self.data.len() - self.pos).min(buf.len());
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn emit_without_init_does_not_panic() {
        emit(&HostEvent::Title {
            title: "no-writer".into(),
        });
    }

    #[test]
    fn write_keys_event_is_one_contract_json_line() {
        let mut buf = Vec::new();
        write_event(
            &mut buf,
            &HostEvent::Keys {
                text: "A".into(),
            },
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text.bytes().filter(|&b| b == b'\n').count(), 1);
        let value: serde_json::Value = serde_json::from_str(text.trim_end()).unwrap();
        assert_eq!(value["event"], "keys");
        assert_eq!(value["text"], "A");
    }

    #[test]
    fn write_focus_event_is_one_contract_json_line() {
        let mut buf = Vec::new();
        write_event(
            &mut buf,
            &HostEvent::Focus {
                owner: FocusOwner::Browser,
                next: None,
            },
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text.bytes().filter(|&b| b == b'\n').count(), 1);
        let value: serde_json::Value = serde_json::from_str(text.trim_end()).unwrap();
        assert_eq!(value["event"], "focus");
        assert_eq!(value["owner"], "browser");
        assert!(value.get("next").is_none());

        let mut buf = Vec::new();
        write_event(
            &mut buf,
            &HostEvent::Focus {
                owner: FocusOwner::App,
                next: Some(false),
            },
        )
        .unwrap();
        let value: serde_json::Value =
            serde_json::from_str(String::from_utf8(buf).unwrap().trim_end()).unwrap();
        assert_eq!(value["event"], "focus");
        assert_eq!(value["owner"], "app");
        assert_eq!(value["next"], false);
    }

    #[test]
    fn unfocus_command_is_snake_case_and_not_focus() {
        let json = serde_json::to_string(&HostCommand::Unfocus).unwrap();
        assert_eq!(json, r#"{"cmd":"unfocus"}"#);
        assert_ne!(json, serde_json::to_string(&HostCommand::Focus).unwrap());
        assert_eq!(
            parse_command_line(r#"{"cmd":"unfocus"}"#),
            Some(HostCommand::Unfocus)
        );
        assert_eq!(
            parse_command_line(r#"{"cmd":"focus"}"#),
            Some(HostCommand::Focus)
        );
    }

    #[test]
    fn activate_command_is_not_focus() {
        let json = serde_json::to_string(&HostCommand::Activate).unwrap();
        assert_eq!(json, r#"{"cmd":"activate"}"#);
        assert_ne!(json, serde_json::to_string(&HostCommand::Focus).unwrap());
        assert_ne!(json, serde_json::to_string(&HostCommand::Unfocus).unwrap());
        assert_eq!(
            parse_command_line(r#"{"cmd":"activate"}"#),
            Some(HostCommand::Activate)
        );
    }

    #[test]
    fn write_event_is_one_contract_json_line() {
        let mut buf = Vec::new();
        write_event(
            &mut buf,
            &HostEvent::Ready {
                cef: "152.0.6+…".into(),
                chromium: "152.0.7977.83".into(),
                api_version: 15200,
                xid: 123456,
            },
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(
            text.ends_with('\n'),
            "protocol frames are newline-terminated"
        );
        assert_eq!(text.bytes().filter(|&b| b == b'\n').count(), 1);
        let value: serde_json::Value = serde_json::from_str(text.trim_end()).unwrap();
        assert_eq!(value["event"], "ready");
        assert_eq!(value["apiVersion"], 15200);
        assert_eq!(value["xid"], 123456);
    }

    #[test]
    fn write_event_escapes_newlines_so_payload_stays_one_line() {
        let mut buf = Vec::new();
        write_event(
            &mut buf,
            &HostEvent::Fatal {
                message: "line1\nline2".into(),
                code: 11,
            },
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text.bytes().filter(|&b| b == b'\n').count(), 1);
        assert!(text.contains("\\n"));
        let value: serde_json::Value = serde_json::from_str(text.trim_end()).unwrap();
        assert_eq!(value["message"], "line1\nline2");
    }

    #[test]
    fn write_event_survives_broken_pipe() {
        let (reader, mut writer) = io::pipe().expect("pipe");
        write_event(
            &mut writer,
            &HostEvent::Title {
                title: "before-close".into(),
            },
        )
        .unwrap();
        drop(reader);
        let err = write_event(
            &mut writer,
            &HostEvent::Title {
                title: "after-peer-gone".into(),
            },
        )
        .expect_err("write to a closed pipe must fail, not panic");
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn emit_swallows_write_errors() {
        // emit() is `let _ = write_event(...)`. A broken pipe must not unwind.
        let (reader, mut writer) = io::pipe().expect("pipe");
        drop(reader);
        assert!(write_event(
            &mut writer,
            &HostEvent::Title {
                title: "gone".into(),
            },
        )
        .is_err());
        emit(&HostEvent::Title {
            title: "no-init".into(),
        });
    }

    #[test]
    fn emit_after_init_survives_broken_pipe() {
        use std::os::fd::{FromRawFd, IntoRawFd};
        let (reader, writer) = io::pipe().expect("pipe");
        drop(reader);
        let file = unsafe { File::from_raw_fd(writer.into_raw_fd()) };
        let _ = WRITER.set(Mutex::new(file));
        emit(&HostEvent::Fatal {
            message: "ade-gone".into(),
            code: 11,
        });
        emit(&HostEvent::Title {
            title: "second-write-also-swallowed".into(),
        });
    }

    #[test]
    fn stdin_eof_posts_close_only() {
        let (cmds, stop) = posted("");
        assert_eq!(stop, StdinStop::Eof);
        assert_eq!(cmds, vec![HostCommand::Close]);
        assert_eq!(exit_code_for_stdin_stop(stop), 0);
    }

    #[test]
    fn stdin_eof_after_commands_posts_single_trailing_close() {
        let (cmds, stop) = posted("{\"cmd\":\"back\"}\n{\"cmd\":\"forward\"}\n");
        assert_eq!(stop, StdinStop::Eof);
        assert_eq!(
            cmds,
            vec![HostCommand::Back, HostCommand::Forward, HostCommand::Close]
        );
        assert_eq!(exit_code_for_stdin_stop(stop), 0);
    }

    #[test]
    fn stdin_eof_is_orderly_exit_zero() {
        // CONTRACT §4.4: EOF = ADE died → close → exit 0 (§4.7). Never fatal.
        for (input, expected_stop) in [
            ("", StdinStop::Eof),
            ("{\"cmd\":\"show\"}\n", StdinStop::Eof),
            ("   \n", StdinStop::Eof),
        ] {
            let (cmds, stop) = posted(input);
            assert_eq!(stop, expected_stop);
            assert_eq!(exit_code_for_stdin_stop(stop), 0);
            assert_eq!(cmds.last(), Some(&HostCommand::Close));
            assert_eq!(cmds.iter().filter(|c| **c == HostCommand::Close).count(), 1);
        }
    }

    #[test]
    fn stdin_read_error_posts_close_and_is_exit_zero() {
        let (cmds, stop) = posted_reader(FailAfter {
            data: b"{\"cmd\":\"hide\"}\n",
            pos: 0,
        });
        assert_eq!(stop, StdinStop::ReadError);
        assert_eq!(exit_code_for_stdin_stop(stop), 0);
        assert_eq!(cmds, vec![HostCommand::Hide, HostCommand::Close]);
    }

    #[test]
    fn command_after_close_is_ignored() {
        let (cmds, stop) = posted(
            "{\"cmd\":\"navigate\",\"url\":\"https://ok.test\"}\n\
             {\"cmd\":\"close\"}\n\
             {\"cmd\":\"navigate\",\"url\":\"https://evil.test\"}\n\
             {\"cmd\":\"reload\",\"ignoreCache\":true}\n",
        );
        assert_eq!(stop, StdinStop::CloseCommand);
        assert_eq!(exit_code_for_stdin_stop(stop), 0);
        assert_eq!(
            cmds,
            vec![
                HostCommand::Navigate {
                    url: "https://ok.test".into()
                },
                HostCommand::Close
            ]
        );
    }

    #[test]
    fn close_then_eof_does_not_double_close() {
        let (cmds, stop) = posted("{\"cmd\":\"close\"}\n");
        assert_eq!(stop, StdinStop::CloseCommand);
        assert_eq!(cmds, vec![HostCommand::Close]);
    }

    #[test]
    fn close_is_case_sensitive_snake_case() {
        let (cmds, stop) = posted("{\"cmd\":\"Close\"}\n{\"cmd\":\"CLOSE\"}\n");
        assert_eq!(stop, StdinStop::Eof);
        assert_eq!(cmds, vec![HostCommand::Close]);
    }

    #[test]
    fn partial_json_without_newline_is_not_a_command() {
        // `BufRead::lines` would yield this last fragment. We must not execute it.
        let (cmds, stop) = posted("{\"cmd\":\"navigate\",\"url\":\"https://truncated.test\"}");
        assert_eq!(stop, StdinStop::Eof);
        assert_eq!(cmds, vec![HostCommand::Close]);
    }

    #[test]
    fn partial_json_prefix_then_eof_is_not_a_command() {
        let (cmds, stop) = posted("{\"cmd\":\"na");
        assert_eq!(cmds, vec![HostCommand::Close]);
        assert_eq!(stop, StdinStop::Eof);
    }

    #[test]
    fn partial_second_line_after_valid_is_dropped() {
        let (cmds, stop) = posted("{\"cmd\":\"back\"}\n{\"cmd\":\"for");
        assert_eq!(cmds, vec![HostCommand::Back, HostCommand::Close]);
        assert_eq!(stop, StdinStop::Eof);
    }

    #[test]
    fn split_writes_assemble_into_one_command() {
        let bytes = b"{\"cmd\":\"back\"}\n";
        let (cmds, stop) = posted_reader(OneByte(bytes));
        assert_eq!(stop, StdinStop::Eof);
        assert_eq!(cmds, vec![HostCommand::Back, HostCommand::Close]);
    }

    #[test]
    fn split_writes_across_object_then_newline() {
        let bytes = b"{\"cmd\":\"navigate\",\"url\":\"https://chunked.test\"}\n";
        let (cmds, stop) = posted_reader(OneByte(bytes));
        assert_eq!(
            cmds,
            vec![
                HostCommand::Navigate {
                    url: "https://chunked.test".into()
                },
                HostCommand::Close
            ]
        );
        assert_eq!(stop, StdinStop::Eof);
    }

    #[test]
    fn broken_complete_json_lines_are_ignored() {
        let input = concat!(
            "{\"cmd\":\"navigate\"}\n",
            "{\"cmd\":\"back\",}\n",
            "{not json}\n",
            "{\"cmd\":0}\n",
            "{\"cmd\":\"back\"}{\"cmd\":\"forward\"}\n",
            "{\"cmd\":\"back\",\"cmd\":\"forward\"}\n",
            "[]\n",
            "null\n",
            "{\"cmd\":\"unknown_future\"}\n",
            "{\"cmd\":\"set_bounds\",\"x\":0,\"y\":0,\"w\":1}\n",
        );
        let (cmds, stop) = posted(input);
        assert_eq!(stop, StdinStop::Eof);
        assert_eq!(cmds, vec![HostCommand::Close]);
    }

    #[test]
    fn whitespace_blank_and_crlf_lines() {
        let (cmds, stop) = posted("  \n\n\r\n{\"cmd\":\"stop\"}\r\n  {\"cmd\":\"show\"}  \r\n");
        assert_eq!(
            cmds,
            vec![HostCommand::Stop, HostCommand::Show, HostCommand::Close]
        );
        assert_eq!(stop, StdinStop::Eof);
    }

    #[test]
    fn reload_ignore_cache_defaults_false() {
        let (cmds, _) = posted("{\"cmd\":\"reload\"}\n");
        assert_eq!(
            cmds,
            vec![
                HostCommand::Reload {
                    ignore_cache: false
                },
                HostCommand::Close
            ]
        );
    }

    #[test]
    fn contract_commands_round_trip() {
        let cmds = [
            HostCommand::Navigate {
                url: "https://idioteque.test".into(),
            },
            HostCommand::Back,
            HostCommand::Forward,
            HostCommand::Stop,
            HostCommand::Reload { ignore_cache: true },
            HostCommand::SetBounds {
                x: 0,
                y: 36,
                w: 1200,
                h: 700,
            },
            HostCommand::Show,
            HostCommand::Hide,
            HostCommand::Focus,
            HostCommand::Unfocus,
            HostCommand::Activate,
            HostCommand::Devtools,
            HostCommand::Close,
        ];
        let mut stdin = String::new();
        for cmd in &cmds {
            stdin.push_str(&serde_json::to_string(cmd).unwrap());
            stdin.push('\n');
        }
        let (got, stop) = posted(&stdin);
        assert_eq!(stop, StdinStop::CloseCommand);
        assert_eq!(got, cmds);
    }

    #[test]
    fn cursor_without_final_newline_matches_partial_rule() {
        let data = b"{\"cmd\":\"focus\"}";
        let (cmds, stop) = posted_reader(Cursor::new(data));
        assert_eq!(cmds, vec![HostCommand::Close]);
        assert_eq!(stop, StdinStop::Eof);
    }

    #[test]
    fn concurrent_posts_from_pump_stay_ordered() {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let sink_thread = sink.clone();
        let stop = pump_stdin(
            BufReader::new(&b"{\"cmd\":\"back\"}\n{\"cmd\":\"forward\"}\n"[..]),
            move |cmd| sink_thread.lock().unwrap().push(cmd),
        );
        assert_eq!(stop, StdinStop::Eof);
        assert_eq!(
            *sink.lock().unwrap(),
            vec![HostCommand::Back, HostCommand::Forward, HostCommand::Close]
        );
    }
}
