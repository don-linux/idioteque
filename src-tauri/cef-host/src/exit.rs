//! Exit codes from CONTRACT.md §4.7 and a single fatal path that emits JSON first.

use std::io::Write;
use std::process;

use crate::protocol::{self, HostEvent};

pub const OK: i32 = 0;
pub const BAD_ARGS: i32 = 2;
pub const API_INCOMPAT: i32 = 10;
pub const INIT_FAILED: i32 = 11;
pub const HEALTH_TIMEOUT: i32 = 12;
pub const VERSION_MISMATCH: i32 = 13;
pub const BAD_SLOT: i32 = 14;
pub const SANDBOX: i32 = 15;
pub const NO_DISPLAY: i32 = 16;

/// Failure that becomes a `fatal` protocol event and a process exit.
/// Extracted so unit tests can inspect JSON + codes without `process::exit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FatalError {
    pub code: i32,
    pub message: String,
}

impl FatalError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Contract `{"event":"fatal","message":"…","code":N}` — no I/O, no exit.
pub fn fatal_event(error: &FatalError) -> HostEvent {
    HostEvent::Fatal {
        message: error.message.clone(),
        code: error.code,
    }
}

/// One JSON object, no trailing newline. Same payload `protocol::emit` writes.
#[cfg(test)]
pub fn encode_fatal(error: &FatalError) -> String {
    serde_json::to_string(&fatal_event(error)).expect("fatal event is always serializable")
}

/// Emit `{"event":"fatal",...}` on the protocol fd and terminate. Never returns.
pub fn fatal(code: i32, message: impl Into<String>) -> ! {
    abort_fatal(FatalError::new(code, message));
}

pub fn abort_fatal(error: FatalError) -> ! {
    eprintln!("cef-host fatal {}: {}", error.code, error.message);
    protocol::emit(&fatal_event(&error));
    // Best-effort flush of the original stderr as well.
    let _ = std::io::stderr().flush();
    process::exit(error.code);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract_codes() -> [(i32, i32); 9] {
        [
            (OK, 0),
            (BAD_ARGS, 2),
            (API_INCOMPAT, 10),
            (INIT_FAILED, 11),
            (HEALTH_TIMEOUT, 12),
            (VERSION_MISMATCH, 13),
            (BAD_SLOT, 14),
            (SANDBOX, 15),
            (NO_DISPLAY, 16),
        ]
    }

    #[test]
    fn contract_exit_codes_match_section_4_7() {
        for (constant, expected) in contract_codes() {
            assert_eq!(constant, expected);
        }
        let mut codes: Vec<i32> = contract_codes().into_iter().map(|(c, _)| c).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), 9, "exit codes must be unique");
    }

    #[test]
    fn fatal_json_shape_for_each_nonzero_code() {
        for code in [2, 10, 11, 12, 13, 14, 15, 16] {
            let error = FatalError::new(code, format!("boom-{code}"));
            let json = encode_fatal(&error);
            assert_eq!(
                json,
                format!(r#"{{"event":"fatal","message":"boom-{code}","code":{code}}}"#)
            );
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(value["event"], "fatal");
            assert_eq!(value["code"], code);
            assert_eq!(value["message"], format!("boom-{code}"));
            let obj = value.as_object().unwrap();
            assert_eq!(obj.len(), 3);
            assert!(value["code"].is_i64());
            assert!(!value["code"].is_string());
        }
    }

    #[test]
    fn fatal_json_is_one_line_and_escapes_payload() {
        let error = FatalError::new(
            BAD_ARGS,
            "line1\nline2\t\"quoted\" \\ slash \u{1f4a5}",
        );
        let json = encode_fatal(&error);
        assert!(!json.contains('\n'), "payload newline must be escaped: {json}");
        assert!(!json.contains('\t'), "payload tab must be escaped: {json}");
        assert!(json.contains("\\n"));
        assert!(json.contains("\\\"quoted\\\""));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value["message"],
            "line1\nline2\t\"quoted\" \\ slash \u{1f4a5}"
        );
        assert_eq!(value["code"], 2);
    }

    #[test]
    fn fatal_event_matches_encode() {
        let error = FatalError::new(INIT_FAILED, "cef_initialize failed");
        match fatal_event(&error) {
            HostEvent::Fatal { message, code } => {
                assert_eq!(code, 11);
                assert_eq!(message, "cef_initialize failed");
            }
            other => panic!("expected Fatal, got {other:?}"),
        }
    }

    #[test]
    fn empty_message_still_emits_valid_fatal() {
        let json = encode_fatal(&FatalError::new(NO_DISPLAY, ""));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["event"], "fatal");
        assert_eq!(value["message"], "");
        assert_eq!(value["code"], 16);
    }
}
