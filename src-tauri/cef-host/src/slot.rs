//! Slot `manifest.json` validation and libcef `version_info` cross-check.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::exit::{self, fatal};

pub const HOST_API_VERSION: u32 = 15200;

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    #[allow(dead_code)]
    pub schema: Option<u32>,
    #[serde(rename = "cefVersion")]
    pub cef_version: String,
    #[serde(rename = "chromiumVersion")]
    pub chromium_version: String,
    pub platform: String,
    #[serde(rename = "apiVersionMin")]
    pub api_version_min: i32,
    #[serde(rename = "apiVersionLast")]
    #[allow(dead_code)]
    pub api_version_last: Option<i32>,
    #[serde(default)]
    pub files: Vec<ManifestFile>,
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const REQUIRED: &[&str] = &[
    "libcef.so",
    "icudtl.dat",
    "v8_context_snapshot.bin",
    "resources.pak",
    "chrome_100_percent.pak",
    "chrome_200_percent.pak",
    "locales/en-US.pak",
    "libEGL.so",
    "libGLESv2.so",
    "libvk_swiftshader.so",
    "libvulkan.so.1",
    "vk_swiftshader_icd.json",
    "chrome-sandbox",
];

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
const REQUIRED: &[&str] = &[];

pub fn expected_platform() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "linux64"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "linuxarm64"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "windows64"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "macosx64"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "macosarm64"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    {
        "unknown"
    }
}

/// Compiled-in CEF version string from `cef_dll_sys` (no libcef call).
pub fn compiled_cef_version() -> String {
    let bytes = cef::sys::CEF_VERSION;
    std::ffi::CStr::from_bytes_until_nul(bytes)
        .ok()
        .and_then(|s| s.to_str().ok())
        .unwrap_or("152.0.6+g708dc14+chromium-152.0.7977.83")
        .to_string()
}

unsafe extern "C" {
    fn cef_version_info(entry: std::os::raw::c_int) -> std::os::raw::c_int;
}

/// Running libcef version parts via `cef_version_info`.
pub fn running_versions() -> (String, String) {
    unsafe {
        let major = cef_version_info(0);
        let minor = cef_version_info(1);
        let patch = cef_version_info(2);
        let cmaj = cef_version_info(4);
        let cmin = cef_version_info(5);
        let cbuild = cef_version_info(6);
        let cpatch = cef_version_info(7);
        (
            format!("{major}.{minor}.{patch}"),
            format!("{cmaj}.{cmin}.{cbuild}.{cpatch}"),
        )
    }
}

pub fn validate(slot: &Path) -> Manifest {
    let manifest_path = slot.join("manifest.json");
    let raw = fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        fatal(
            exit::BAD_SLOT,
            format!("cannot read {}: {e}", manifest_path.display()),
        )
    });
    let manifest: Manifest = serde_json::from_str(&raw).unwrap_or_else(|e| {
        fatal(exit::BAD_SLOT, format!("invalid manifest.json: {e}"))
    });

    if manifest.platform != expected_platform() {
        fatal(
            exit::BAD_SLOT,
            format!(
                "slot platform {} != host {}",
                manifest.platform,
                expected_platform()
            ),
        );
    }

    if manifest.api_version_min as u32 > HOST_API_VERSION {
        fatal(
            exit::API_INCOMPAT,
            format!(
                "slot apiVersionMin {} > host {}",
                manifest.api_version_min, HOST_API_VERSION
            ),
        );
    }

    for rel in REQUIRED {
        let p = join_slot(slot, rel);
        if !p.is_file() {
            fatal(
                exit::BAD_SLOT,
                format!("required file missing: {}", p.display()),
            );
        }
    }

    for entry in &manifest.files {
        let p = join_slot(slot, &entry.path);
        if !p.is_file() {
            fatal(
                exit::BAD_SLOT,
                format!("manifest file missing: {}", p.display()),
            );
        }
        if let Some(size) = entry.size {
            let got = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if got != size {
                fatal(
                    exit::BAD_SLOT,
                    format!(
                        "size mismatch for {}: manifest {size} disk {got}",
                        p.display()
                    ),
                );
            }
        }
    }

    let (cef_num, chrome) = running_versions();
    let manifest_cef_num = manifest
        .cef_version
        .split('+')
        .next()
        .unwrap_or(&manifest.cef_version);
    if manifest_cef_num != cef_num {
        fatal(
            exit::VERSION_MISMATCH,
            format!(
                "libcef version_info {cef_num} != manifest.cefVersion {}",
                manifest.cef_version
            ),
        );
    }
    if manifest.chromium_version != chrome {
        fatal(
            exit::VERSION_MISMATCH,
            format!(
                "libcef chrome {chrome} != manifest.chromiumVersion {}",
                manifest.chromium_version
            ),
        );
    }

    manifest
}

fn join_slot(slot: &Path, rel: &str) -> PathBuf {
    let mut p = slot.to_path_buf();
    for part in rel.split('/') {
        p.push(part);
    }
    p
}
