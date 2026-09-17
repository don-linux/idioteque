//! `--idq-*` argument parsing. Unknown flags are ignored (Chromium / CEF switches).

use std::env;
use std::path::PathBuf;

use crate::exit::{self, fatal, FatalError};

#[derive(Clone, Debug, PartialEq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            w: 800,
            h: 600,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostArgs {
    pub info: bool,
    pub cef_dir: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    pub parent: Option<u64>,
    pub bounds: Bounds,
    pub scale: Option<f64>,
    pub url: String,
    pub health_check: bool,
    pub no_sandbox: bool,
    pub log_file: Option<PathBuf>,
    /// Chromium switches from `IDIOTEQUE_CEF_ARGS`, stored as written (`--switch`).
    /// `app.rs` strips a leading `--` before `append_switch`.
    pub extra_switches: Vec<String>,
}

/// OS bits that `parse()` reads. Tests inject these so they do not race on process env.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseEnv {
    pub no_sandbox: bool,
    pub extra_args: Option<String>,
}

impl ParseEnv {
    pub fn from_os() -> Self {
        Self {
            no_sandbox: env_flag_true(env::var("IDIOTEQUE_CEF_NO_SANDBOX").ok().as_deref()),
            extra_args: env::var("IDIOTEQUE_CEF_ARGS").ok(),
        }
    }
}

pub fn env_flag_true(raw: Option<&str>) -> bool {
    raw.is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub fn split_extra_args(raw: &str) -> Vec<String> {
    raw.split_whitespace()
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

impl HostArgs {
    pub fn parse() -> Self {
        Self::try_parse(env::args(), &ParseEnv::from_os())
            .unwrap_or_else(|error| fatal(error.code, error.message))
    }

    /// `argv[0]` is the program name (same as `env::args()`). Never calls `process::exit`.
    pub fn try_parse<I, S>(argv: I, env: &ParseEnv) -> Result<Self, FatalError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut args = HostArgs {
            info: false,
            cef_dir: None,
            cache_dir: None,
            parent: None,
            bounds: Bounds::default(),
            scale: None,
            url: "about:blank".to_string(),
            health_check: false,
            no_sandbox: env.no_sandbox,
            log_file: None,
            extra_switches: env
                .extra_args
                .as_deref()
                .map(split_extra_args)
                .unwrap_or_default(),
        };

        let argv: Vec<String> = argv.into_iter().map(|s| s.as_ref().to_string()).collect();
        let mut i = 1;
        while i < argv.len() {
            let a = argv[i].as_str();
            if let Some(rest) = a.strip_prefix("--idq-") {
                let (key, inline) = match rest.split_once('=') {
                    Some((k, v)) => (k, Some(v.to_string())),
                    None => (rest, None),
                };
                let take = |inline: Option<String>, i: &mut usize| -> Result<String, FatalError> {
                    if let Some(v) = inline {
                        return Ok(v);
                    }
                    *i += 1;
                    argv.get(*i).cloned().ok_or_else(|| {
                        FatalError::new(exit::BAD_ARGS, format!("missing value for --idq-{key}"))
                    })
                };
                match key {
                    "info" => args.info = true,
                    "health-check" => args.health_check = true,
                    "no-sandbox" => args.no_sandbox = true,
                    "cef-dir" => args.cef_dir = Some(PathBuf::from(take(inline, &mut i)?)),
                    "cache-dir" => args.cache_dir = Some(PathBuf::from(take(inline, &mut i)?)),
                    "parent" => {
                        let raw = take(inline, &mut i)?;
                        args.parent = Some(parse_xid(&raw).ok_or_else(|| {
                            FatalError::new(exit::BAD_ARGS, format!("invalid --idq-parent: {raw}"))
                        })?);
                    }
                    "bounds" => {
                        let raw = take(inline, &mut i)?;
                        args.bounds = parse_bounds(&raw).ok_or_else(|| {
                            FatalError::new(exit::BAD_ARGS, format!("invalid --idq-bounds: {raw}"))
                        })?;
                    }
                    "scale" => {
                        let raw = take(inline, &mut i)?;
                        args.scale = Some(parse_scale(&raw).ok_or_else(|| {
                            FatalError::new(exit::BAD_ARGS, format!("invalid --idq-scale: {raw}"))
                        })?);
                    }
                    "url" => args.url = take(inline, &mut i)?,
                    "log" => args.log_file = Some(PathBuf::from(take(inline, &mut i)?)),
                    _ => {
                        // Unknown --idq-* : ignore (do not consume a following token).
                    }
                }
            }
            // Unknown / Chromium args: ignore.
            i += 1;
        }

        if args.health_check {
            args.url = "about:blank".to_string();
        }

        Ok(args)
    }

    pub fn require_slot_and_cache(&self) -> (std::path::PathBuf, std::path::PathBuf) {
        self.try_require_slot_and_cache()
            .unwrap_or_else(|error| fatal(error.code, error.message))
    }

    pub fn try_require_slot_and_cache(&self) -> Result<(PathBuf, PathBuf), FatalError> {
        let cef_dir = self
            .cef_dir
            .clone()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| FatalError::new(exit::BAD_ARGS, "missing --idq-cef-dir"))?;
        let cache_dir = self
            .cache_dir
            .clone()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| FatalError::new(exit::BAD_ARGS, "missing --idq-cache-dir"))?;
        Ok((cef_dir, cache_dir))
    }

    pub fn log_path(&self, cache_dir: &std::path::Path) -> std::path::PathBuf {
        self.log_file
            .clone()
            .unwrap_or_else(|| cache_dir.join("cef-host.log"))
    }
}

pub fn parse_xid(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

pub fn parse_bounds(raw: &str) -> Option<Bounds> {
    let parts: Vec<&str> = raw.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    Some(Bounds {
        x: parts[0].trim().parse().ok()?,
        y: parts[1].trim().parse().ok()?,
        w: parts[2].trim().parse().ok()?,
        h: parts[3].trim().parse().ok()?,
    })
}

pub fn parse_scale(raw: &str) -> Option<f64> {
    let value = raw.parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(flags: &[&str]) -> HostArgs {
        parse_env(flags, &ParseEnv::default())
    }

    fn parse_env(flags: &[&str], env: &ParseEnv) -> HostArgs {
        let mut argv = vec!["cef-host".to_string()];
        argv.extend(flags.iter().map(|s| (*s).to_string()));
        HostArgs::try_parse(argv, env).expect("parse")
    }

    fn parse_err(flags: &[&str]) -> FatalError {
        let mut argv = vec!["cef-host".to_string()];
        argv.extend(flags.iter().map(|s| (*s).to_string()));
        HostArgs::try_parse(argv, &ParseEnv::default()).expect_err("expected BAD_ARGS")
    }

    #[test]
    fn inline_equals_matches_separate_tokens() {
        let inline = parse(&[
            "--idq-cef-dir=/slot",
            "--idq-cache-dir=/cache",
            "--idq-parent=0x1a2b",
            "--idq-bounds=-4,8,800,600",
            "--idq-scale=1.25",
            "--idq-url=https://x.test/?a=1&b=2",
            "--idq-log=/tmp/chromium.log",
        ]);
        let tokens = parse(&[
            "--idq-cef-dir",
            "/slot",
            "--idq-cache-dir",
            "/cache",
            "--idq-parent",
            "0x1a2b",
            "--idq-bounds",
            "-4,8,800,600",
            "--idq-scale",
            "1.25",
            "--idq-url",
            "https://x.test/?a=1&b=2",
            "--idq-log",
            "/tmp/chromium.log",
        ]);
        assert_eq!(inline, tokens);
        assert_eq!(inline.cef_dir, Some(PathBuf::from("/slot")));
        assert_eq!(inline.cache_dir, Some(PathBuf::from("/cache")));
        assert_eq!(inline.parent, Some(0x1a2b));
        assert_eq!(
            inline.bounds,
            Bounds {
                x: -4,
                y: 8,
                w: 800,
                h: 600
            }
        );
        assert_eq!(inline.scale, Some(1.25));
        assert_eq!(inline.url, "https://x.test/?a=1&b=2");
        assert_eq!(inline.log_file, Some(PathBuf::from("/tmp/chromium.log")));
    }

    #[test]
    fn mixed_inline_and_tokens() {
        let args = parse(&[
            "--idq-cef-dir=/slot",
            "--idq-cache-dir",
            "/cache",
            "--idq-url=https://ok",
        ]);
        assert_eq!(args.cef_dir, Some(PathBuf::from("/slot")));
        assert_eq!(args.cache_dir, Some(PathBuf::from("/cache")));
        assert_eq!(args.url, "https://ok");
    }

    #[test]
    fn unknown_idq_does_not_consume_next_token() {
        let stolen = parse(&[
            "--idq-unknown",
            "https://stolen.test",
            "--idq-url",
            "https://ok.test",
        ]);
        assert_eq!(stolen.url, "https://ok.test");

        let inline_unknown = parse(&["--idq-unknown=https://stolen.test", "--idq-url=https://ok.test"]);
        assert_eq!(inline_unknown.url, "https://ok.test");

        let near_miss = parse(&["--idq-cef-dir-extra", "/not-slot", "--idq-url", "https://ok"]);
        assert_eq!(near_miss.cef_dir, None);
        assert_eq!(near_miss.url, "https://ok");
    }

    #[test]
    fn chromium_and_non_idq_flags_are_ignored() {
        let args = parse(&[
            "--type=renderer",
            "--disable-gpu",
            "--idq-cef-dir",
            "/slot",
            "--ozone-platform=x11",
            "--idq-cache-dir=/cache",
            "-idq-url",
            "https://not-a-flag",
            "--IDQ-url=https://wrong-case",
        ]);
        assert_eq!(args.cef_dir, Some(PathBuf::from("/slot")));
        assert_eq!(args.cache_dir, Some(PathBuf::from("/cache")));
        assert_eq!(args.url, "about:blank");
    }

    #[test]
    fn xid_hex_and_decimal() {
        assert_eq!(parse_xid("0"), Some(0));
        assert_eq!(parse_xid("12345"), Some(12345));
        assert_eq!(parse_xid("00042"), Some(42));
        assert_eq!(parse_xid("18446744073709551615"), Some(u64::MAX));
        assert_eq!(parse_xid("0x0"), Some(0));
        assert_eq!(parse_xid("0x1a2b"), Some(0x1a2b));
        assert_eq!(parse_xid("0XDEAD"), Some(0xdead));
        assert_eq!(parse_xid("0x00000000000000ff"), Some(255));
        assert_eq!(parse_xid("0xffffffffffffffff"), Some(u64::MAX));
        assert_eq!(parse_xid("  0x10  "), Some(0x10));
        assert_eq!(parse_xid(" 99 "), Some(99));
        assert_eq!(parse(&["--idq-parent=0x1a2b"]).parent, Some(0x1a2b));
        assert_eq!(parse(&["--idq-parent", "12345"]).parent, Some(12345));
        assert_eq!(parse(&["--idq-parent=0"]).parent, Some(0));
    }

    #[test]
    fn xid_invalid() {
        for raw in [
            "",
            "0x",
            "0xG",
            "0x 10",
            "-1",
            "+123",
            "0o12",
            "0b10",
            "deadbeef",
            "123abc",
            "#1a2b",
            "18446744073709551616",
            "0x10000000000000000",
            "1.0",
            "0x10u64",
        ] {
            assert_eq!(parse_xid(raw), None, "xid {raw:?}");
            let error = parse_err(&["--idq-parent", raw]);
            assert_eq!(error.code, exit::BAD_ARGS);
            assert!(
                error.message.contains("invalid --idq-parent"),
                "{}",
                error.message
            );
        }
        let empty_inline = parse_err(&["--idq-parent="]);
        assert_eq!(empty_inline.code, exit::BAD_ARGS);
    }

    #[test]
    fn bounds_valid_and_invalid() {
        assert_eq!(
            parse_bounds("0,0,800,600").unwrap(),
            Bounds::default()
        );
        assert_eq!(
            parse_bounds(" -10 , 20 , 1 , 1 ").unwrap(),
            Bounds {
                x: -10,
                y: 20,
                w: 1,
                h: 1
            }
        );
        // Geometry 0 / negative w,h is accepted here; app.rs clamps with .max(1).
        assert_eq!(
            parse_bounds("0,0,0,0").unwrap(),
            Bounds {
                x: 0,
                y: 0,
                w: 0,
                h: 0
            }
        );
        assert_eq!(
            parse_bounds(&format!("{},{},{},{}", i32::MIN, i32::MAX, 1, 1)).unwrap(),
            Bounds {
                x: i32::MIN,
                y: i32::MAX,
                w: 1,
                h: 1
            }
        );
        assert_eq!(parse(&["--idq-bounds", "-10,20,800,600"]).bounds.x, -10);

        for raw in [
            "",
            "1,2,3",
            "1,2,3,4,5",
            "a,b,c,d",
            "1,2,3,",
            ",1,2,3",
            "1.5,2,3,4",
            "1,2,3,4.0",
            "0,0,800",
            "2147483648,0,800,600",
            "0,0,800,600,1",
            "0 0 800 600",
        ] {
            assert_eq!(parse_bounds(raw), None, "bounds {raw:?}");
            let error = parse_err(&["--idq-bounds", raw]);
            assert_eq!(error.code, exit::BAD_ARGS);
            assert!(
                error.message.contains("invalid --idq-bounds"),
                "{}",
                error.message
            );
        }
        assert_eq!(parse_err(&["--idq-bounds="]).code, exit::BAD_ARGS);
    }

    #[test]
    fn scale_rejects_non_finite() {
        assert_eq!(parse_scale("1"), Some(1.0));
        assert_eq!(parse_scale("1.25"), Some(1.25));
        assert_eq!(parse_scale("1e2"), Some(100.0));
        assert_eq!(parse_scale("0"), Some(0.0));
        assert_eq!(parse_scale("-1"), Some(-1.0));
        assert_eq!(parse_scale(" 2.0 "), Some(2.0));
        for raw in ["", "x", "1.2.3", "nan", "NaN", "inf", "INF", "-inf", "infinity"] {
            assert_eq!(parse_scale(raw), None, "scale {raw:?}");
            let error = parse_err(&["--idq-scale", raw]);
            assert_eq!(error.code, exit::BAD_ARGS);
            assert!(error.message.contains("invalid --idq-scale"), "{}", error.message);
        }
    }

    #[test]
    fn missing_value_is_bad_args() {
        for flag in [
            "--idq-cef-dir",
            "--idq-cache-dir",
            "--idq-parent",
            "--idq-bounds",
            "--idq-scale",
            "--idq-url",
            "--idq-log",
        ] {
            let error = parse_err(&[flag]);
            assert_eq!(error.code, exit::BAD_ARGS, "{flag}");
            assert!(
                error.message.starts_with("missing value for --idq-"),
                "{flag}: {}",
                error.message
            );
        }
    }

    #[test]
    fn boolean_flags_do_not_consume_next_token() {
        let args = parse(&[
            "--idq-info",
            "--idq-health-check",
            "--idq-no-sandbox",
            "--idq-url",
            "https://should-be-blank",
        ]);
        assert!(args.info);
        assert!(args.health_check);
        assert!(args.no_sandbox);
        assert_eq!(args.url, "about:blank");
    }

    #[test]
    fn boolean_flags_with_inline_value_still_enable() {
        let args = parse(&[
            "--idq-info=false",
            "--idq-health-check=0",
            "--idq-no-sandbox=0",
        ]);
        assert!(args.info);
        assert!(args.health_check);
        assert!(args.no_sandbox);
    }

    #[test]
    fn health_check_forces_about_blank_even_after_url() {
        let before = parse(&["--idq-url=https://evil.test", "--idq-health-check"]);
        assert!(before.health_check);
        assert_eq!(before.url, "about:blank");
        let after = parse(&["--idq-health-check", "--idq-url=https://evil.test"]);
        assert_eq!(after.url, "about:blank");
    }

    #[test]
    fn last_value_wins() {
        let args = parse(&[
            "--idq-url=https://first.test",
            "--idq-url",
            "https://second.test",
            "--idq-bounds=0,0,1,1",
            "--idq-bounds=2,3,4,5",
            "--idq-parent=1",
            "--idq-parent=0x10",
        ]);
        assert_eq!(args.url, "https://second.test");
        assert_eq!(args.bounds, Bounds { x: 2, y: 3, w: 4, h: 5 });
        assert_eq!(args.parent, Some(0x10));
    }

    #[test]
    fn extra_args_split_on_whitespace_keep_dashes() {
        assert_eq!(
            split_extra_args("  --disable-gpu   --foo=bar  "),
            vec!["--disable-gpu".to_string(), "--foo=bar".to_string()]
        );
        assert_eq!(split_extra_args(""), Vec::<String>::new());
        assert_eq!(split_extra_args("   "), Vec::<String>::new());
        // Quotes are not a grouping syntax.
        assert_eq!(
            split_extra_args("--user-agent=hello world"),
            vec!["--user-agent=hello".to_string(), "world".to_string()]
        );
        let args = parse_env(
            &[],
            &ParseEnv {
                no_sandbox: false,
                extra_args: Some("--disable-gpu --disable-extensions".into()),
            },
        );
        assert_eq!(
            args.extra_switches,
            vec!["--disable-gpu".to_string(), "--disable-extensions".to_string()]
        );
    }

    #[test]
    fn no_sandbox_env_and_flag() {
        assert!(env_flag_true(Some("1")));
        assert!(env_flag_true(Some("true")));
        assert!(env_flag_true(Some("TRUE")));
        assert!(env_flag_true(Some("True")));
        assert!(!env_flag_true(Some("0")));
        assert!(!env_flag_true(Some("false")));
        assert!(!env_flag_true(Some("yes")));
        assert!(!env_flag_true(Some("")));
        assert!(!env_flag_true(None));

        let from_env = parse_env(
            &[],
            &ParseEnv {
                no_sandbox: true,
                extra_args: None,
            },
        );
        assert!(from_env.no_sandbox);
        let from_flag = parse(&["--idq-no-sandbox"]);
        assert!(from_flag.no_sandbox);
        let neither = parse(&[]);
        assert!(!neither.no_sandbox);
    }

    #[test]
    fn info_without_slot_parses_but_require_fails() {
        let args = parse(&["--idq-info"]);
        assert!(args.info);
        assert_eq!(args.cef_dir, None);
        let error = args.try_require_slot_and_cache().unwrap_err();
        assert_eq!(error.code, exit::BAD_ARGS);
        assert_eq!(error.message, "missing --idq-cef-dir");
    }

    #[test]
    fn require_slot_and_cache_rejects_empty_inline() {
        let missing_cache = parse(&["--idq-cef-dir=/slot"]);
        let error = missing_cache.try_require_slot_and_cache().unwrap_err();
        assert_eq!(error.code, exit::BAD_ARGS);
        assert_eq!(error.message, "missing --idq-cache-dir");

        let empty_dirs = parse(&["--idq-cef-dir=", "--idq-cache-dir="]);
        let error = empty_dirs.try_require_slot_and_cache().unwrap_err();
        assert_eq!(error.code, exit::BAD_ARGS);
        assert_eq!(error.message, "missing --idq-cef-dir");
    }

    #[test]
    fn require_ok_and_log_path() {
        let args = parse(&[
            "--idq-cef-dir=/slot",
            "--idq-cache-dir=/cache",
            "--idq-log=/tmp/custom.log",
        ]);
        let (cef, cache) = args.try_require_slot_and_cache().unwrap();
        assert_eq!(cef, PathBuf::from("/slot"));
        assert_eq!(cache, PathBuf::from("/cache"));
        assert_eq!(args.log_path(&cache), PathBuf::from("/tmp/custom.log"));
        let default_log = parse(&["--idq-cef-dir=/slot", "--idq-cache-dir=/cache"]);
        assert_eq!(
            default_log.log_path(&PathBuf::from("/cache")),
            PathBuf::from("/cache/cef-host.log")
        );
    }

    #[test]
    fn defaults_without_flags() {
        let args = parse(&[]);
        assert!(!args.info);
        assert!(!args.health_check);
        assert!(!args.no_sandbox);
        assert_eq!(args.url, "about:blank");
        assert_eq!(args.bounds, Bounds::default());
        assert_eq!(args.scale, None);
        assert_eq!(args.parent, None);
        assert!(args.extra_switches.is_empty());
    }

    #[test]
    fn take_consumes_a_following_flag_as_value() {
        // Existing ADE spawn always passes a real value. Lock current take() semantics.
        let args = parse(&["--idq-url", "--idq-health-check"]);
        assert_eq!(args.url, "--idq-health-check");
        assert!(!args.health_check);
    }
}
