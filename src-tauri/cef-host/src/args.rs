//! `--idq-*` argument parsing. Unknown flags are ignored (Chromium / CEF switches).

use std::env;
use std::path::PathBuf;

use crate::exit::{self, fatal};

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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
    /// Chromium switches from `IDIOTEQUE_CEF_ARGS` (without the leading `--` stored raw).
    pub extra_switches: Vec<String>,
}

impl HostArgs {
    pub fn parse() -> Self {
        let mut args = HostArgs {
            info: false,
            cef_dir: None,
            cache_dir: None,
            parent: None,
            bounds: Bounds::default(),
            scale: None,
            url: "about:blank".to_string(),
            health_check: false,
            no_sandbox: env::var("IDIOTEQUE_CEF_NO_SANDBOX")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            log_file: None,
            extra_switches: env::var("IDIOTEQUE_CEF_ARGS")
                .map(|s| {
                    s.split_whitespace()
                        .filter(|t| !t.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        };

        let argv: Vec<String> = env::args().collect();
        let mut i = 1;
        while i < argv.len() {
            let a = argv[i].as_str();
            if let Some(rest) = a.strip_prefix("--idq-") {
                let (key, inline) = match rest.split_once('=') {
                    Some((k, v)) => (k, Some(v.to_string())),
                    None => (rest, None),
                };
                let take = |inline: Option<String>, i: &mut usize| -> String {
                    if let Some(v) = inline {
                        return v;
                    }
                    *i += 1;
                    argv.get(*i).cloned().unwrap_or_else(|| {
                        fatal(exit::BAD_ARGS, format!("missing value for --idq-{key}"))
                    })
                };
                match key {
                    "info" => args.info = true,
                    "health-check" => args.health_check = true,
                    "no-sandbox" => args.no_sandbox = true,
                    "cef-dir" => args.cef_dir = Some(PathBuf::from(take(inline, &mut i))),
                    "cache-dir" => args.cache_dir = Some(PathBuf::from(take(inline, &mut i))),
                    "parent" => {
                        let raw = take(inline, &mut i);
                        args.parent = Some(parse_xid(&raw).unwrap_or_else(|| {
                            fatal(exit::BAD_ARGS, format!("invalid --idq-parent: {raw}"))
                        }));
                    }
                    "bounds" => {
                        let raw = take(inline, &mut i);
                        args.bounds = parse_bounds(&raw).unwrap_or_else(|| {
                            fatal(exit::BAD_ARGS, format!("invalid --idq-bounds: {raw}"))
                        });
                    }
                    "scale" => {
                        let raw = take(inline, &mut i);
                        args.scale = Some(raw.parse::<f64>().unwrap_or_else(|_| {
                            fatal(exit::BAD_ARGS, format!("invalid --idq-scale: {raw}"))
                        }));
                    }
                    "url" => args.url = take(inline, &mut i),
                    "log" => args.log_file = Some(PathBuf::from(take(inline, &mut i))),
                    _ => {
                        // Unknown --idq-* : ignore (do not consume a following token).
                        if inline.is_none() {
                            // nothing
                        }
                    }
                }
            }
            // Unknown / Chromium args: ignore.
            i += 1;
        }

        if args.health_check {
            args.url = "about:blank".to_string();
        }

        args
    }

    pub fn require_slot_and_cache(&self) -> (std::path::PathBuf, std::path::PathBuf) {
        let cef_dir = self
            .cef_dir
            .clone()
            .unwrap_or_else(|| fatal(exit::BAD_ARGS, "missing --idq-cef-dir"));
        let cache_dir = self
            .cache_dir
            .clone()
            .unwrap_or_else(|| fatal(exit::BAD_ARGS, "missing --idq-cache-dir"));
        (cef_dir, cache_dir)
    }

    pub fn log_path(&self, cache_dir: &std::path::Path) -> std::path::PathBuf {
        self.log_file
            .clone()
            .unwrap_or_else(|| cache_dir.join("cef-host.log"))
    }
}

fn parse_xid(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

fn parse_bounds(raw: &str) -> Option<Bounds> {
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
