//! Protocolo JSON línea a línea entre el ADE y `cef-host` (contrato 4.3 / 4.4).

use serde::{Deserialize, Serialize};

/// Eventos host → ADE (stdout). `Exit` lo sintetiza el ADE al terminar el proceso.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum HostEvent {
    #[serde(rename_all = "camelCase")]
    Ready {
        cef: String,
        chromium: String,
        api_version: u32,
        xid: u64,
    },
    #[serde(rename_all = "camelCase")]
    Nav {
        url: String,
        can_go_back: bool,
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
    Focus {
        owner: FocusOwner,
        #[serde(default)]
        next: Option<bool>,
    },
    RenderCrashed {
        status: String,
    },
    #[serde(rename_all = "camelCase")]
    Health {
        ok: bool,
        cef: String,
        chromium: String,
        api_version: u32,
    },
    Fatal {
        message: String,
        code: i32,
    },
    #[serde(rename_all = "camelCase")]
    Info {
        api_version: u32,
        cef_compiled: String,
    },
    Exit {
        code: i32,
    },
    /// Evento futuro o desconocido: no tumba el lector.
    #[serde(other)]
    Unknown,
}

/// Comandos ADE → host (stdin).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum HostCommand {
    Navigate {
        url: String,
    },
    Back,
    Forward,
    Stop,
    #[serde(rename_all = "camelCase")]
    Reload {
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
    Devtools,
    Close,
}

/// Who should own the keyboard after a host `focus` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FocusOwner {
    App,
    Browser,
}

/// Tope de una línea host→ADE. Holgado respecto al máximo de URL de Chromium
/// (~2 MiB) más el sobre JSON. El lector del ADE ya trata `Err` como no fatal
/// (`host.rs` registra y sigue); `ready`/`health` caben de sobra.
pub const MAX_EVENT_LINE: usize = 4 * 1024 * 1024;

pub fn parse_event(line: &str) -> Result<HostEvent, String> {
    if line.len() > MAX_EVENT_LINE {
        return Err(format!("evento CEF demasiado largo ({} bytes)", line.len()));
    }
    let line = line.trim();
    if line.is_empty() {
        return Err("evento CEF vacío".to_string());
    }
    serde_json::from_str(line).map_err(|error| format!("No se pudo parsear el evento CEF: {error}"))
}

/// Una línea JSON con `\n` final.
pub fn encode_command(cmd: &HostCommand) -> String {
    let mut line = serde_json::to_string(cmd).unwrap_or_else(|_| "{\"cmd\":\"stop\"}".to_string());
    line.push('\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_contract_events() {
        let ready = parse_event(
            r#"{"event":"ready","cef":"152.0.6+…","chromium":"152.0.7977.83","apiVersion":15200,"xid":123456}"#,
        )
        .unwrap();
        assert_eq!(
            ready,
            HostEvent::Ready {
                cef: "152.0.6+…".into(),
                chromium: "152.0.7977.83".into(),
                api_version: 15200,
                xid: 123456,
            }
        );

        let nav = parse_event(
            r#"{"event":"nav","url":"https://…","canGoBack":true,"canGoForward":false,"loading":true}"#,
        )
        .unwrap();
        assert_eq!(
            nav,
            HostEvent::Nav {
                url: "https://…".into(),
                can_go_back: true,
                can_go_forward: false,
                loading: true,
            }
        );

        assert_eq!(
            parse_event(r#"{"event":"title","title":"…"}"#).unwrap(),
            HostEvent::Title {
                title: "…".into()
            }
        );
        assert_eq!(
            parse_event(r#"{"event":"load-end","status":200}"#).unwrap(),
            HostEvent::LoadEnd { status: 200 }
        );
        assert_eq!(
            parse_event(
                r#"{"event":"load-error","code":-105,"text":"ERR_NAME_NOT_RESOLVED","url":"…"}"#
            )
            .unwrap(),
            HostEvent::LoadError {
                code: -105,
                text: "ERR_NAME_NOT_RESOLVED".into(),
                url: "…".into(),
            }
        );
        assert_eq!(
            parse_event(r#"{"event":"shortcut","chord":"ctrl+b"}"#).unwrap(),
            HostEvent::Shortcut {
                chord: "ctrl+b".into()
            }
        );
        assert_eq!(
            parse_event(r#"{"event":"focus","owner":"browser"}"#).unwrap(),
            HostEvent::Focus {
                owner: FocusOwner::Browser,
                next: None,
            }
        );
        assert_eq!(
            parse_event(r#"{"event":"focus","owner":"app","next":true}"#).unwrap(),
            HostEvent::Focus {
                owner: FocusOwner::App,
                next: Some(true),
            }
        );
        assert_eq!(
            parse_event(r#"{"event":"focus","owner":"app","next":false}"#).unwrap(),
            HostEvent::Focus {
                owner: FocusOwner::App,
                next: Some(false),
            }
        );
        assert_eq!(
            parse_event(
                r#"{"event":"health","ok":true,"cef":"…","chromium":"…","apiVersion":15200}"#
            )
            .unwrap(),
            HostEvent::Health {
                ok: true,
                cef: "…".into(),
                chromium: "…".into(),
                api_version: 15200,
            }
        );
        assert_eq!(
            parse_event(r#"{"event":"fatal","message":"…","code":11}"#).unwrap(),
            HostEvent::Fatal {
                message: "…".into(),
                code: 11,
            }
        );
        assert_eq!(
            parse_event(r#"{"event":"render-crashed","status":"…"}"#).unwrap(),
            HostEvent::RenderCrashed {
                status: "…".into()
            }
        );
        assert_eq!(
            parse_event(r#"{"event":"info","apiVersion":15200,"cefCompiled":"152.0.6+…"}"#)
                .unwrap(),
            HostEvent::Info {
                api_version: 15200,
                cef_compiled: "152.0.6+…".into(),
            }
        );
        assert_eq!(
            parse_event(r#"{"event":"exit","code":15}"#).unwrap(),
            HostEvent::Exit { code: 15 }
        );
    }

    fn assert_parse_err(line: &str) {
        match parse_event(line) {
            Ok(event) => panic!("se esperaba Err, se obtuvo {event:?} para {line:?}"),
            Err(msg) => {
                assert!(
                    msg == "evento CEF vacío"
                        || msg.starts_with("No se pudo parsear el evento CEF:")
                        || msg.starts_with("evento CEF demasiado largo"),
                    "mensaje inesperado para {line:?}: {msg}"
                );
                assert!(
                    msg.len() < 400,
                    "el error no debe volcar la línea: {} bytes",
                    msg.len()
                );
            }
        }
    }

    #[test]
    fn unknown_event_does_not_break_the_reader() {
        let parsed = parse_event(r#"{"event":"future-thing","foo":1}"#).unwrap();
        assert_eq!(parsed, HostEvent::Unknown);
        assert!(parse_event("{not json").is_err());
        assert!(parse_event("").is_err());
    }

    #[test]
    fn broken_json_is_err_never_unknown() {
        for line in [
            "",
            "   ",
            "\t",
            "\r",
            "\n",
            "{not json",
            "{",
            "}",
            "[",
            "]",
            "[]",
            "null",
            "true",
            "false",
            "0",
            "\"title\"",
            "{\"event\":",
            "{\"event\":\"title\"",
            "{\"event\":\"title\",}",
            "{\"event\":\"title\",\"title\":}",
            "{\"event\":\"title\",\"title\":\"x\",}",
            "{'event':'title','title':'x'}",
            "{event:\"title\",\"title\":\"x\"}",
            "{\"event\": title, \"title\": \"x\"}",
            "{\"event\":\"title\",\"title\":\"x\"}{",
            "{\"event\":\"title\",\"title\":\"x\"} junk",
            "{\"event\":\"title\",\"title\":\"x\"}{\"event\":\"title\",\"title\":\"y\"}",
            "<event>title</event>",
            "event=title",
            "{\"EVENT\":\"title\",\"title\":\"x\"}",
            "{\"event\":null}",
            "{\"event\":1}",
            "{\"event\":true}",
            "{\"event\":[\"title\"]}",
            "{\"event\":{\"title\":\"x\"}}",
            "// {\"event\":\"title\",\"title\":\"x\"}",
            "{\"event\":\"title\",\"title\":\"x\" // hi}",
        ] {
            assert_parse_err(line);
        }
    }

    #[test]
    fn known_event_with_bad_payload_is_err_not_unknown() {
        for line in [
            r#"{"event":"ready"}"#,
            r#"{"event":"ready","cef":"152.0.6","chromium":"152.0.7977.83","apiVersion":15200}"#,
            r#"{"event":"ready","cef":"x","chromium":"y","apiVersion":15200,"xid":-1}"#,
            r#"{"event":"ready","cef":"x","chromium":"y","apiVersion":15200,"xid":123456.5}"#,
            r#"{"event":"nav","url":"https://x","canGoBack":true,"canGoForward":false}"#,
            r#"{"event":"nav","url":1,"canGoBack":true,"canGoForward":false,"loading":true}"#,
            r#"{"event":"nav","url":"https://x","canGoBack":true,"canGoForward":false,"loading":1}"#,
            r#"{"event":"title"}"#,
            r#"{"event":"title","title":null}"#,
            r#"{"event":"load-end"}"#,
            r#"{"event":"load-end","status":"200"}"#,
            r#"{"event":"load-error","code":-105,"text":"ERR","url":null}"#,
            r#"{"event":"shortcut"}"#,
            r#"{"event":"focus"}"#,
            r#"{"event":"focus","owner":"nope"}"#,
            r#"{"event":"focus","next":true}"#,
            r#"{"event":"render-crashed"}"#,
            r#"{"event":"health","ok":"true","cef":"x","chromium":"y","apiVersion":15200}"#,
            r#"{"event":"fatal","message":"boom"}"#,
            r#"{"event":"info","apiVersion":15200}"#,
            r#"{"event":"exit"}"#,
            r#"{"event":"exit","code":"15"}"#,
        ] {
            assert_parse_err(line);
        }
    }

    #[test]
    fn unknown_event_names_are_unknown_not_err() {
        for line in [
            r#"{"event":"future-thing"}"#,
            r#"{"event":"future-thing","foo":1}"#,
            r#"{"event":"loadend","status":200}"#,
            r#"{"event":"load_end","status":200}"#,
            r#"{"event":"Ready","cef":"x","chromium":"y","apiVersion":15200,"xid":1}"#,
            r#"{"event":"NAV","url":"https://x","canGoBack":true,"canGoForward":false,"loading":true}"#,
            r#"{"event":""}"#,
            r#"{"event":"nav "}"#,
            r#"{"event":"unknown"}"#,
            r#"{"event":"devtools-opened"}"#,
        ] {
            assert_eq!(parse_event(line).expect(line), HostEvent::Unknown, "{line}");
        }

        let bulky = format!(
            r#"{{"event":"future-thing","blob":"{}","nested":{{"a":1}}}}"#,
            "z".repeat(4096)
        );
        assert_eq!(parse_event(&bulky).unwrap(), HostEvent::Unknown);
    }

    #[test]
    fn known_event_ignores_unknown_fields() {
        let ready = parse_event(
            r#"{"event":"ready","cef":"152.0.6","chromium":"1","apiVersion":15200,"xid":9,"extra":true}"#,
        )
        .unwrap();
        assert_eq!(
            ready,
            HostEvent::Ready {
                cef: "152.0.6".into(),
                chromium: "1".into(),
                api_version: 15200,
                xid: 9,
            }
        );
    }

    #[test]
    fn whitespace_around_json_still_parses() {
        assert_eq!(
            parse_event("  {\"event\":\"title\",\"title\":\"x\"}  \r").unwrap(),
            HostEvent::Title { title: "x".into() }
        );
        assert_eq!(
            parse_event("\t{\"event\":\"title\",\"title\":\"x\"}\n").unwrap(),
            HostEvent::Title { title: "x".into() }
        );
    }

    #[test]
    fn utf8_and_escaped_newline_in_strings() {
        let title = parse_event(r#"{"event":"title","title":"日本語 🎧 \nlinea"}"#).unwrap();
        assert_eq!(
            title,
            HostEvent::Title {
                title: "日本語 🎧 \nlinea".into()
            }
        );
        let nav = parse_event(
            r#"{"event":"nav","url":"https://ex.com/á?q=1","canGoBack":false,"canGoForward":false,"loading":false}"#,
        )
        .unwrap();
        match nav {
            HostEvent::Nav { url, .. } => assert_eq!(url, "https://ex.com/á?q=1"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn duplicate_keys_are_err_not_unknown() {
        // serde_json rechaza campos duplicados; no se tragan como Unknown ni last-wins.
        for line in [
            r#"{"event":"title","title":"first","title":"second"}"#,
            r#"{"event":"future","event":"title","title":"x"}"#,
            r#"{"event":"title","title":"x","event":"future"}"#,
            r#"{"event":"nav","url":"https://x","url":"https://y","canGoBack":false,"canGoForward":false,"loading":false}"#,
        ] {
            assert_parse_err(line);
        }
    }

    #[test]
    fn huge_valid_line_under_cap_parses() {
        let title = "á".repeat(32 * 1024);
        let json = serde_json::json!({ "event": "title", "title": title }).to_string();
        assert!(json.len() < MAX_EVENT_LINE);
        assert_eq!(parse_event(&json).unwrap(), HostEvent::Title { title });

        let url = format!("https://example.test/{}", "p".repeat(256 * 1024));
        let json = serde_json::json!({
            "event": "nav",
            "url": url,
            "canGoBack": false,
            "canGoForward": false,
            "loading": true,
        })
        .to_string();
        assert!(json.len() < MAX_EVENT_LINE);
        match parse_event(&json).unwrap() {
            HostEvent::Nav { url: parsed, .. } => assert_eq!(parsed, url),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn huge_broken_line_is_err_without_panic() {
        let garbage = "{".repeat(1024 * 1024);
        assert!(garbage.len() < MAX_EVENT_LINE);
        assert_parse_err(&garbage);

        let mut nested = String::from("{\"event\":\"future\"");
        for _ in 0..200 {
            nested.push_str(",\"n\":[");
        }
        nested.push_str(&"1".to_string());
        for _ in 0..200 {
            nested.push(']');
        }
        nested.push('}');
        assert!(nested.len() < MAX_EVENT_LINE);
        assert_parse_err(&nested);
    }

    #[test]
    fn huge_line_over_cap_is_err_without_dumping_payload() {
        let over = "x".repeat(MAX_EVENT_LINE + 1);
        let err = parse_event(&over).expect_err("línea enorme debe rechazarse");
        assert!(err.starts_with("evento CEF demasiado largo"), "{err}");
        assert!(err.contains(&(MAX_EVENT_LINE + 1).to_string()));
        assert!(err.len() < 80, "{err}");
        assert!(!err.contains("xxxx"));

        let mut valid_but_huge = String::from(r#"{"event":"title","title":""#);
        valid_but_huge.push_str(&"y".repeat(MAX_EVENT_LINE));
        valid_but_huge.push_str(r#""}"#);
        assert!(valid_but_huge.len() > MAX_EVENT_LINE);
        let err = parse_event(&valid_but_huge).expect_err("JSON válido pero enorme");
        assert!(err.starts_with("evento CEF demasiado largo"));
        assert!(!err.contains("yyy"));
    }

    #[test]
    fn encode_contract_commands() {
        assert_eq!(
            encode_command(&HostCommand::Navigate { url: "…".into() }).trim_end(),
            r#"{"cmd":"navigate","url":"…"}"#
        );
        assert_eq!(
            encode_command(&HostCommand::Back).trim_end(),
            r#"{"cmd":"back"}"#
        );
        assert_eq!(
            encode_command(&HostCommand::Forward).trim_end(),
            r#"{"cmd":"forward"}"#
        );
        assert_eq!(
            encode_command(&HostCommand::Stop).trim_end(),
            r#"{"cmd":"stop"}"#
        );
        assert_eq!(
            encode_command(&HostCommand::Reload {
                ignore_cache: false
            })
            .trim_end(),
            r#"{"cmd":"reload","ignoreCache":false}"#
        );
        assert_eq!(
            encode_command(&HostCommand::SetBounds {
                x: 0,
                y: 36,
                w: 1200,
                h: 700
            })
            .trim_end(),
            r#"{"cmd":"set_bounds","x":0,"y":36,"w":1200,"h":700}"#
        );
        assert_eq!(
            encode_command(&HostCommand::Show).trim_end(),
            r#"{"cmd":"show"}"#
        );
        assert_eq!(
            encode_command(&HostCommand::Hide).trim_end(),
            r#"{"cmd":"hide"}"#
        );
        assert_eq!(
            encode_command(&HostCommand::Focus).trim_end(),
            r#"{"cmd":"focus"}"#
        );
        assert_eq!(
            encode_command(&HostCommand::Unfocus).trim_end(),
            r#"{"cmd":"unfocus"}"#
        );
        assert_ne!(
            encode_command(&HostCommand::Unfocus),
            encode_command(&HostCommand::Focus)
        );
        assert_eq!(
            encode_command(&HostCommand::Devtools).trim_end(),
            r#"{"cmd":"devtools"}"#
        );
        let close = encode_command(&HostCommand::Close);
        assert_eq!(close, "{\"cmd\":\"close\"}\n");
        assert!(close.ends_with('\n'));
    }

    #[test]
    fn parse_command_round_trip() {
        let cmd: HostCommand =
            serde_json::from_str(r#"{"cmd":"reload","ignoreCache":true}"#).unwrap();
        assert_eq!(cmd, HostCommand::Reload { ignore_cache: true });
    }

    #[test]
    fn broken_and_unknown_commands_are_err() {
        for line in [
            "",
            "{not json",
            r#"{"cmd":"teleport"}"#,
            r#"{"cmd":"navigate"}"#,
            r#"{"cmd":"reload","ignoreCache":"yes"}"#,
            r#"{"cmd":"set_bounds","x":0,"y":0,"w":1}"#,
        ] {
            assert!(serde_json::from_str::<HostCommand>(line).is_err(), "{line}");
        }
    }

    #[test]
    fn encode_huge_navigate_stays_one_line() {
        let url = format!("https://example.test/{}", "u".repeat(64 * 1024));
        let line = encode_command(&HostCommand::Navigate { url: url.clone() });
        assert!(line.ends_with('\n'));
        assert_eq!(line.matches('\n').count(), 1);
        let parsed: HostCommand = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(parsed, HostCommand::Navigate { url });
    }
}
