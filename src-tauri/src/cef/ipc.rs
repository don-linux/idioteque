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
    Devtools,
    Close,
}

pub fn parse_event(line: &str) -> Result<HostEvent, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("evento CEF vacío".to_string());
    }
    serde_json::from_str(line)
        .map_err(|error| format!("No se pudo parsear el evento CEF: {error}"))
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
            HostEvent::Title { title: "…".into() }
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
            parse_event(
                r#"{"event":"info","apiVersion":15200,"cefCompiled":"152.0.6+…"}"#
            )
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

    #[test]
    fn unknown_event_does_not_break_the_reader() {
        let parsed = parse_event(r#"{"event":"future-thing","foo":1}"#).unwrap();
        assert_eq!(parsed, HostEvent::Unknown);
        assert!(parse_event("{not json").is_err());
        assert!(parse_event("").is_err());
    }

    #[test]
    fn encode_contract_commands() {
        assert_eq!(
            encode_command(&HostCommand::Navigate {
                url: "…".into()
            })
            .trim_end(),
            r#"{"cmd":"navigate","url":"…"}"#
        );
        assert_eq!(encode_command(&HostCommand::Back).trim_end(), r#"{"cmd":"back"}"#);
        assert_eq!(
            encode_command(&HostCommand::Forward).trim_end(),
            r#"{"cmd":"forward"}"#
        );
        assert_eq!(encode_command(&HostCommand::Stop).trim_end(), r#"{"cmd":"stop"}"#);
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
        assert_eq!(encode_command(&HostCommand::Show).trim_end(), r#"{"cmd":"show"}"#);
        assert_eq!(encode_command(&HostCommand::Hide).trim_end(), r#"{"cmd":"hide"}"#);
        assert_eq!(encode_command(&HostCommand::Focus).trim_end(), r#"{"cmd":"focus"}"#);
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
        assert_eq!(
            cmd,
            HostCommand::Reload {
                ignore_cache: true
            }
        );
    }
}
