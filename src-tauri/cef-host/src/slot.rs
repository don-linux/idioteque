//! Slot `manifest.json` validation and libcef `version_info` cross-check.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::exit::{self, fatal, FatalError};

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

pub fn required_files() -> &'static [&'static str] {
    REQUIRED
}

pub fn expected_platform() -> &'static str {
    "linux64"
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

/// Filesystem + manifest checks. Does **not** call libcef (`running_versions`).
pub fn check_slot(slot: &Path) -> Result<Manifest, FatalError> {
    let manifest_path = slot.join("manifest.json");
    let raw = fs::read_to_string(&manifest_path).map_err(|e| {
        FatalError::new(
            exit::BAD_SLOT,
            format!("cannot read {}: {e}", manifest_path.display()),
        )
    })?;
    let manifest: Manifest = serde_json::from_str(&raw)
        .map_err(|e| FatalError::new(exit::BAD_SLOT, format!("invalid manifest.json: {e}")))?;

    if manifest.platform != expected_platform() {
        return Err(FatalError::new(
            exit::BAD_SLOT,
            format!(
                "slot platform {} != host {}",
                manifest.platform,
                expected_platform()
            ),
        ));
    }

    if manifest.api_version_min as u32 > HOST_API_VERSION {
        return Err(FatalError::new(
            exit::API_INCOMPAT,
            format!(
                "slot apiVersionMin {} > host {}",
                manifest.api_version_min, HOST_API_VERSION
            ),
        ));
    }

    for rel in required_files() {
        let p = join_slot(slot, rel)?;
        if !p.is_file() {
            return Err(FatalError::new(
                exit::BAD_SLOT,
                format!("required file missing: {}", p.display()),
            ));
        }
    }

    for entry in &manifest.files {
        let p = join_slot(slot, &entry.path)?;
        if !p.is_file() {
            return Err(FatalError::new(
                exit::BAD_SLOT,
                format!("manifest file missing: {}", p.display()),
            ));
        }
        if let Some(size) = entry.size {
            let got = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if got != size {
                return Err(FatalError::new(
                    exit::BAD_SLOT,
                    format!(
                        "size mismatch for {}: manifest {size} disk {got}",
                        p.display()
                    ),
                ));
            }
        }
    }

    Ok(manifest)
}

/// Compare loaded libcef numbers against the manifest. No FFI.
pub fn check_versions(manifest: &Manifest, cef_num: &str, chrome: &str) -> Result<(), FatalError> {
    let manifest_cef_num = manifest
        .cef_version
        .split('+')
        .next()
        .unwrap_or(&manifest.cef_version);
    if manifest_cef_num != cef_num {
        return Err(FatalError::new(
            exit::VERSION_MISMATCH,
            format!(
                "libcef version_info {cef_num} != manifest.cefVersion {}",
                manifest.cef_version
            ),
        ));
    }
    if manifest.chromium_version != chrome {
        return Err(FatalError::new(
            exit::VERSION_MISMATCH,
            format!(
                "libcef chrome {chrome} != manifest.chromiumVersion {}",
                manifest.chromium_version
            ),
        ));
    }
    Ok(())
}

pub fn validate(slot: &Path) -> Manifest {
    let manifest = check_slot(slot).unwrap_or_else(|error| fatal(error.code, error.message));
    let (cef_num, chrome) = running_versions();
    check_versions(&manifest, &cef_num, &chrome)
        .unwrap_or_else(|error| fatal(error.code, error.message));
    manifest
}

pub fn join_slot(slot: &Path, rel: &str) -> Result<PathBuf, FatalError> {
    let rel = rel.trim();
    if rel.is_empty() {
        return Err(FatalError::new(exit::BAD_SLOT, "empty manifest file path"));
    }
    if Path::new(rel).is_absolute() {
        return Err(FatalError::new(
            exit::BAD_SLOT,
            format!("manifest path escapes slot: {rel}"),
        ));
    }
    let mut p = slot.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains('\0') {
            return Err(FatalError::new(
                exit::BAD_SLOT,
                format!("manifest path escapes slot: {rel}"),
            ));
        }
        p.push(part);
    }
    if !p.starts_with(slot) {
        return Err(FatalError::new(
            exit::BAD_SLOT,
            format!("manifest path escapes slot: {rel}"),
        ));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_N: AtomicU64 = AtomicU64::new(0);

    fn temp_slot(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "idq-slot-test-{}-{}-{tag}",
            std::process::id(),
            TEMP_N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("slot dir");
        dir
    }

    fn write_required(dir: &Path) {
        for name in required_files() {
            let path = dir.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(&path, b"x").expect("write required");
        }
    }

    fn write_manifest(dir: &Path, body: &str) {
        fs::write(dir.join("manifest.json"), body).expect("manifest");
    }

    fn ok_manifest_json(files: &str) -> String {
        format!(
            r#"{{
                "cefVersion": "152.0.6+g708dc14+chromium-152.0.7977.83",
                "chromiumVersion": "152.0.7977.83",
                "platform": "{}",
                "apiVersionMin": 13300,
                "apiVersionLast": 15200,
                "files": {files}
            }}"#,
            expected_platform()
        )
    }

    fn sample_manifest(cef: &str, chrome: &str) -> Manifest {
        Manifest {
            schema: Some(1),
            cef_version: cef.to_string(),
            chromium_version: chrome.to_string(),
            platform: expected_platform().to_string(),
            api_version_min: 13300,
            api_version_last: Some(15200),
            files: Vec::new(),
        }
    }

    #[test]
    fn host_api_version_is_15200() {
        assert_eq!(HOST_API_VERSION, 15200);
        assert_eq!(expected_platform(), "linux64");
    }

    #[test]
    fn check_slot_accepts_required_files_without_libcef() {
        let dir = temp_slot("ok");
        write_required(&dir);
        write_manifest(&dir, &ok_manifest_json("[]"));
        let manifest = check_slot(&dir).expect("valid slot");
        assert_eq!(manifest.cef_version, "152.0.6+g708dc14+chromium-152.0.7977.83");
        assert_eq!(manifest.platform, "linux64");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_slot_does_not_compare_libcef_versions() {
        let dir = temp_slot("no-ffi");
        write_required(&dir);
        write_manifest(
            &dir,
            &format!(
                r#"{{
                    "cefVersion": "0.0.0+not-a-real-libcef",
                    "chromiumVersion": "9.9.9.9",
                    "platform": "{}",
                    "apiVersionMin": 0,
                    "files": []
                }}"#,
                expected_platform()
            ),
        );
        check_slot(&dir).expect("filesystem checks only");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_invalid_manifest_is_bad_slot() {
        let missing = temp_slot("no-manifest");
        let error = check_slot(&missing).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        assert!(error.message.contains("cannot read"), "{}", error.message);
        let _ = fs::remove_dir_all(&missing);

        for (tag, body) in [
            ("trunc", "{"),
            ("array", "[]"),
            ("comma", r#"{"cefVersion":"1","chromiumVersion":"1","platform":"linux64","apiVersionMin":1,}"#),
            ("str-min", r#"{"cefVersion":"1","chromiumVersion":"1","platform":"linux64","apiVersionMin":"no"}"#),
            ("no-cef", r#"{"chromiumVersion":"1","platform":"linux64","apiVersionMin":1}"#),
            ("files-obj", r#"{"cefVersion":"1","chromiumVersion":"1","platform":"linux64","apiVersionMin":1,"files":{}}"#),
        ] {
            let dir = temp_slot(tag);
            write_manifest(&dir, body);
            let error = check_slot(&dir).unwrap_err();
            assert_eq!(error.code, exit::BAD_SLOT, "{tag}");
            assert!(
                error.message.contains("invalid manifest.json"),
                "{tag}: {}",
                error.message
            );
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn extra_json_fields_are_ignored() {
        let dir = temp_slot("extra");
        write_required(&dir);
        write_manifest(
            &dir,
            &format!(
                r#"{{
                    "cefVersion": "152.0.6",
                    "chromiumVersion": "152.0.7977.83",
                    "platform": "{}",
                    "apiVersionMin": 13300,
                    "futureField": true,
                    "files": []
                }}"#,
                expected_platform()
            ),
        );
        check_slot(&dir).expect("forward-compat");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn platform_mismatch_is_bad_slot() {
        let dir = temp_slot("win");
        write_required(&dir);
        write_manifest(
            &dir,
            r#"{
                "cefVersion": "152.0.6",
                "chromiumVersion": "152.0.7977.83",
                "platform": "windows64",
                "apiVersionMin": 13300,
                "files": []
            }"#,
        );
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        assert!(error.message.contains("windows64"), "{}", error.message);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_min_above_host_is_incompat() {
        let dir = temp_slot("api");
        write_required(&dir);
        write_manifest(
            &dir,
            &format!(
                r#"{{
                    "cefVersion": "152.0.6",
                    "chromiumVersion": "152.0.7977.83",
                    "platform": "{}",
                    "apiVersionMin": 15201,
                    "files": []
                }}"#,
                expected_platform()
            ),
        );
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::API_INCOMPAT);
        assert!(error.message.contains("15201"), "{}", error.message);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_min_equal_and_below_ok_negative_wraps_to_incompat() {
        for min in [0, 13300, 15200] {
            let dir = temp_slot(&format!("api{min}"));
            write_required(&dir);
            write_manifest(
                &dir,
                &format!(
                    r#"{{
                        "cefVersion": "152.0.6",
                        "chromiumVersion": "152.0.7977.83",
                        "platform": "{}",
                        "apiVersionMin": {min},
                        "files": []
                    }}"#,
                    expected_platform()
                ),
            );
            check_slot(&dir).unwrap_or_else(|e| panic!("min {min}: {e:?}"));
            let _ = fs::remove_dir_all(&dir);
        }
        let dir = temp_slot("api-neg");
        write_required(&dir);
        write_manifest(
            &dir,
            &format!(
                r#"{{
                    "cefVersion": "152.0.6",
                    "chromiumVersion": "152.0.7977.83",
                    "platform": "{}",
                    "apiVersionMin": -1,
                    "files": []
                }}"#,
                expected_platform()
            ),
        );
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::API_INCOMPAT);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_required_file_is_bad_slot() {
        let dir = temp_slot("miss-req");
        write_required(&dir);
        fs::remove_file(dir.join("libcef.so")).unwrap();
        write_manifest(&dir, &ok_manifest_json("[]"));
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        assert!(
            error.message.contains("required file missing"),
            "{}",
            error.message
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn required_directory_instead_of_file_is_bad_slot() {
        let dir = temp_slot("req-dir");
        write_required(&dir);
        fs::remove_file(dir.join("libcef.so")).unwrap();
        fs::create_dir(dir.join("libcef.so")).unwrap();
        write_manifest(&dir, &ok_manifest_json("[]"));
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn listed_file_missing_and_size_mismatch() {
        let dir = temp_slot("listed");
        write_required(&dir);
        write_manifest(
            &dir,
            &ok_manifest_json(r#"[{"path":"extra.bin","size":2}]"#),
        );
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        assert!(
            error.message.contains("manifest file missing"),
            "{}",
            error.message
        );

        fs::write(dir.join("extra.bin"), b"x").unwrap();
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        assert!(error.message.contains("size mismatch"), "{}", error.message);

        fs::write(dir.join("extra.bin"), b"xy").unwrap();
        check_slot(&dir).expect("size matches");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn listed_size_none_skips_length_check() {
        let dir = temp_slot("nosize");
        write_required(&dir);
        fs::write(dir.join("extra.bin"), b"whatever").unwrap();
        write_manifest(&dir, &ok_manifest_json(r#"[{"path":"extra.bin"}]"#));
        check_slot(&dir).expect("no size");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_escape_dotdot_and_absolute_are_bad_slot() {
        let dir = temp_slot("escape");
        write_required(&dir);
        let outside = dir.parent().unwrap().join(format!(
            "idq-slot-outside-{}",
            std::process::id()
        ));
        fs::write(&outside, b"x").unwrap();

        write_manifest(
            &dir,
            &ok_manifest_json(&format!(
                r#"[{{"path":"../{}","size":1}}]"#,
                outside.file_name().unwrap().to_string_lossy()
            )),
        );
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        assert!(
            error.message.contains("escapes slot"),
            "{}",
            error.message
        );

        write_manifest(
            &dir,
            &ok_manifest_json(&format!(
                r#"[{{"path":"{}","size":1}}]"#,
                outside.display()
            )),
        );
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        assert!(
            error.message.contains("escapes slot"),
            "{}",
            error.message
        );

        write_manifest(&dir, &ok_manifest_json(r#"[{"path":"","size":1}]"#));
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        assert!(
            error.message.contains("empty manifest file path"),
            "{}",
            error.message
        );

        write_manifest(
            &dir,
            &ok_manifest_json(r#"[{"path":"locales/../../etc/passwd"}]"#),
        );
        let error = check_slot(&dir).unwrap_err();
        assert_eq!(error.code, exit::BAD_SLOT);
        assert!(error.message.contains("escapes slot"), "{}", error.message);

        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn join_slot_keeps_nested_required_paths() {
        let slot = Path::new("/tmp/slot");
        assert_eq!(
            join_slot(slot, "locales/en-US.pak").unwrap(),
            PathBuf::from("/tmp/slot/locales/en-US.pak")
        );
        assert_eq!(
            join_slot(slot, "./libcef.so").unwrap(),
            PathBuf::from("/tmp/slot/libcef.so")
        );
        assert!(join_slot(slot, "..").is_err());
        assert!(join_slot(slot, "/etc/passwd").is_err());
    }

    #[test]
    fn versions_match_strips_plus_suffix() {
        let manifest = sample_manifest(
            "152.0.6+g708dc14+chromium-152.0.7977.83",
            "152.0.7977.83",
        );
        check_versions(&manifest, "152.0.6", "152.0.7977.83").unwrap();
        let plain = sample_manifest("152.0.6", "152.0.7977.83");
        check_versions(&plain, "152.0.6", "152.0.7977.83").unwrap();
    }

    #[test]
    fn versions_mismatch_is_exit_13() {
        let manifest = sample_manifest(
            "152.0.6+g708dc14+chromium-152.0.7977.83",
            "152.0.7977.83",
        );
        let cef = check_versions(&manifest, "152.0.7", "152.0.7977.83").unwrap_err();
        assert_eq!(cef.code, exit::VERSION_MISMATCH);
        assert!(cef.message.contains("152.0.7"), "{}", cef.message);
        assert!(
            cef.message.contains("152.0.6+g708dc14"),
            "{}",
            cef.message
        );

        let chrome = check_versions(&manifest, "152.0.6", "1.2.3.4").unwrap_err();
        assert_eq!(chrome.code, exit::VERSION_MISMATCH);
        assert!(chrome.message.contains("1.2.3.4"), "{}", chrome.message);
        assert!(
            chrome.message.contains("152.0.7977.83"),
            "{}",
            chrome.message
        );
    }

    #[test]
    fn compiled_cef_version_is_nonempty() {
        let version = compiled_cef_version();
        assert!(!version.is_empty());
        assert!(version.contains('.'), "{version}");
    }
}
