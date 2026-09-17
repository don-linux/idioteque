//! Extracción del tarball `minimal` al layout plano de un slot CEF.

use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

use bzip2::read::MultiBzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;

use super::index::IndexFile;
use super::manifest::{ManifestFile, SlotManifest, SlotSource};

const COPY_BUF: usize = 64 * 1024;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExtractReport {
    pub files: usize,
    pub bytes: u64,
}

/// Extrae `Release/*`, `Resources/*`, los headers `include/cef_*.h` y
/// `LICENSE.txt` al layout plano de `slot_dir`. Recrea el directorio.
pub fn extract_runtime(tarball: &Path, slot_dir: &Path) -> Result<ExtractReport, String> {
    if slot_dir.exists() {
        fs::remove_dir_all(slot_dir)
            .map_err(|error| format!("No se pudo vaciar `{}`: {error}", slot_dir.display()))?;
    }
    fs::create_dir_all(slot_dir)
        .map_err(|error| format!("No se pudo crear `{}`: {error}", slot_dir.display()))?;

    let file = File::open(tarball)
        .map_err(|error| format!("No se pudo abrir `{}`: {error}", tarball.display()))?;
    let decoder = MultiBzDecoder::new(BufReader::new(file));
    let mut archive = Archive::new(decoder);
    archive.set_preserve_permissions(true);

    let mut files = 0usize;
    let mut bytes = 0u64;

    let entries = archive
        .entries()
        .map_err(|error| format!("No se pudo leer el tarball CEF: {error}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| format!("Entrada tar inválida: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("Ruta tar inválida: {error}"))?
            .into_owned();

        reject_unsafe_path(&path)?;

        let Some(mapped) = map_entry_path(&path) else {
            continue;
        };
        if mapped.as_os_str().is_empty() {
            continue;
        }

        let kind = entry.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        if !kind.is_file() {
            continue;
        }

        let out_path = slot_dir.join(&mapped);
        if !out_path.starts_with(slot_dir) {
            return Err(format!("La entrada `{}` escapa del slot", path.display()));
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("No se pudo crear `{}`: {error}", parent.display()))?;
        }

        let mut out = File::create(&out_path)
            .map_err(|error| format!("No se pudo crear `{}`: {error}", out_path.display()))?;
        let copied = copy_entry(&mut entry, &mut out)?;
        out.flush()
            .map_err(|error| format!("No se pudo escribir `{}`: {error}", out_path.display()))?;
        drop(out);

        apply_unix_mode(&out_path, &mapped, entry.header().mode().ok())?;

        files += 1;
        bytes += copied;
    }

    Ok(ExtractReport { files, bytes })
}

pub fn parse_api_versions(header: &str) -> Result<(u32, u32), String> {
    let min = define_trailing_u32(header, "CEF_API_VERSION_MIN")
        .ok_or_else(|| "No se encontró CEF_API_VERSION_MIN".to_string())?;
    let last = define_trailing_u32(header, "CEF_API_VERSION_LAST")
        .ok_or_else(|| "No se encontró CEF_API_VERSION_LAST".to_string())?;
    Ok((min, last))
}

pub fn parse_cef_version(header: &str) -> Result<(String, String), String> {
    let cef = define_quoted(header, "CEF_VERSION")
        .ok_or_else(|| "No se encontró CEF_VERSION".to_string())?;
    let chromium = chrome_version_from_defines(header)
        .unwrap_or_else(|| super::version::chromium_from(&cef).unwrap_or_default());
    if chromium.is_empty() {
        return Err("No se pudo derivar la versión de Chromium".to_string());
    }
    Ok((cef, chromium))
}

pub fn build_manifest(
    slot_dir: &Path,
    cef_version: &str,
    chromium_version: &str,
    platform: &str,
    api_min: u32,
    api_last: u32,
    archive: &IndexFile,
    stripped: bool,
) -> Result<SlotManifest, String> {
    let mut files = walk_files(slot_dir, slot_dir)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(SlotManifest {
        schema: 1,
        cef_version: cef_version.to_string(),
        chromium_version: chromium_version.to_string(),
        platform: platform.to_string(),
        api_version_min: api_min,
        api_version_last: api_last,
        source: SlotSource::Downloaded,
        archive_name: archive.name.clone(),
        archive_sha1: archive.sha1.clone(),
        archive_size: archive.size,
        stripped,
        files,
        verified: false,
        verified_at: None,
        created_at: created_at_now(),
    })
}

fn created_at_now() -> String {
    super::state::now_rfc3339()
}

fn reject_unsafe_path(path: &Path) -> Result<(), String> {
    if path.is_absolute() {
        return Err(format!("Ruta absoluta rechazada: `{}`", path.display()));
    }
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(format!("Ruta con `..` rechazada: `{}`", path.display()));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(format!("Ruta absoluta rechazada: `{}`", path.display()));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

fn map_entry_path(path: &Path) -> Option<PathBuf> {
    let mut normals: Vec<&std::ffi::OsStr> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normals.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if normals.len() < 2 {
        // Tras quitar el directorio raíz del tarball, LICENSE.txt queda en 1 componente.
        if normals.len() == 1 && normals[0] == "LICENSE.txt" {
            return Some(PathBuf::from("LICENSE.txt"));
        }
        return None;
    }
    let _top = normals.remove(0);
    if normals.is_empty() {
        return None;
    }

    let first = normals[0].to_string_lossy();
    match first.as_ref() {
        "Release" => {
            if normals.len() < 2 {
                return None;
            }
            Some(normals[1..].iter().collect())
        }
        "Resources" => {
            if normals.len() < 2 {
                return None;
            }
            Some(normals[1..].iter().collect())
        }
        "include" => {
            if normals.len() != 2 {
                return None;
            }
            let name = normals[1].to_string_lossy();
            if name == "cef_api_versions.h" || name == "cef_version.h" {
                Some(PathBuf::from("include").join(normals[1]))
            } else {
                None
            }
        }
        "LICENSE.txt" if normals.len() == 1 => Some(PathBuf::from("LICENSE.txt")),
        _ => None,
    }
}

fn copy_entry(entry: &mut tar::Entry<impl Read>, out: &mut File) -> Result<u64, String> {
    let mut buf = vec![0u8; COPY_BUF];
    let mut total = 0u64;
    loop {
        let n = entry
            .read(&mut buf)
            .map_err(|error| format!("No se pudo leer una entrada del tarball: {error}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])
            .map_err(|error| format!("No se pudo extraer el runtime CEF: {error}"))?;
        total += n as u64;
    }
    Ok(total)
}

fn apply_unix_mode(path: &Path, relative: &Path, tar_mode: Option<u32>) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let mode = if force_exec(name) {
            0o755
        } else {
            tar_mode.unwrap_or(0o644) & 0o7777
        };
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
            format!(
                "No se pudieron ajustar permisos de `{}`: {error}",
                path.display()
            )
        })?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, relative, tar_mode);
    }
    Ok(())
}

fn force_exec(name: &str) -> bool {
    name == "chrome-sandbox" || name == "libcef.so" || name.contains(".so")
}

fn define_trailing_u32(header: &str, name: &str) -> Option<u32> {
    for line in header.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("#define") else {
            continue;
        };
        let rest = rest.trim();
        let Some(after) = rest.strip_prefix(name) else {
            continue;
        };
        if !after.is_empty() && !after.starts_with(char::is_whitespace) {
            continue;
        }
        let token = rest.split_whitespace().last()?;
        let digits: String = token
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if digits.is_empty() {
            continue;
        }
        return digits.parse().ok();
    }
    None
}

fn define_quoted(header: &str, name: &str) -> Option<String> {
    for line in header.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("#define") else {
            continue;
        };
        let rest = rest.trim();
        let Some(after) = rest.strip_prefix(name) else {
            continue;
        };
        if !after.is_empty() && !after.starts_with(char::is_whitespace) {
            continue;
        }
        let start = rest.find('"')?;
        let rest = &rest[start + 1..];
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    None
}

fn chrome_version_from_defines(header: &str) -> Option<String> {
    let major = define_plain_u32(header, "CHROME_VERSION_MAJOR")?;
    let minor = define_plain_u32(header, "CHROME_VERSION_MINOR")?;
    let build = define_plain_u32(header, "CHROME_VERSION_BUILD")?;
    let patch = define_plain_u32(header, "CHROME_VERSION_PATCH")?;
    Some(format!("{major}.{minor}.{build}.{patch}"))
}

fn define_plain_u32(header: &str, name: &str) -> Option<u32> {
    for line in header.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("#define") else {
            continue;
        };
        let rest = rest.trim();
        let Some(after) = rest.strip_prefix(name) else {
            continue;
        };
        if !after.starts_with(char::is_whitespace) {
            continue;
        }
        let token = after.split_whitespace().next()?;
        return token.parse().ok();
    }
    None
}

fn walk_files(root: &Path, current: &Path) -> Result<Vec<ManifestFile>, String> {
    let mut out = Vec::new();
    let entries = fs::read_dir(current)
        .map_err(|error| format!("No se pudo leer `{}`: {error}", current.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("No se pudo leer el slot: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("No se pudo leer `{}`: {error}", path.display()))?;
        if file_type.is_dir() {
            out.extend(walk_files(root, &path)?);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|_| format!("Ruta fuera del slot: `{}`", path.display()))?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str == "manifest.json" {
            continue;
        }
        let (size, sha256) = hash_file(&path)?;
        out.push(ManifestFile {
            path: rel_str,
            size,
            sha256,
        });
    }
    Ok(out)
}

fn hash_file(path: &Path) -> Result<(u64, String), String> {
    let mut file = File::open(path)
        .map_err(|error| format!("No se pudo leer `{}`: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; COPY_BUF];
    let mut size = 0u64;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|error| format!("No se pudo leer `{}`: {error}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    Ok((size, hex::encode(hasher.finalize())))
}

/// Cabeceras mínimas válidas para tarballs sintéticos / tests del updater.
#[cfg(test)]
pub fn sample_api_versions_h(min: u32, last: u32) -> String {
    format!(
        "#define CEF_API_VERSION_MIN CEF_API_VERSION_{min}\n#define CEF_API_VERSION_LAST CEF_API_VERSION_{last}\n"
    )
}

#[cfg(test)]
pub fn sample_cef_version_h(cef_version: &str, chromium: &str) -> String {
    let parts: Vec<&str> = chromium.split('.').collect();
    let (major, minor, build, patch) = match parts.as_slice() {
        [a, b, c, d] => (*a, *b, *c, *d),
        _ => ("0", "0", "0", "0"),
    };
    format!(
        "#define CEF_VERSION \"{cef_version}\"\n#define CHROME_VERSION_MAJOR {major}\n#define CHROME_VERSION_MINOR {minor}\n#define CHROME_VERSION_BUILD {build}\n#define CHROME_VERSION_PATCH {patch}\n"
    )
}

#[cfg(test)]
pub(crate) fn write_synthetic_tarball(
    dest: &Path,
    top: &str,
    files: &[(&str, &[u8], u32)],
) -> Result<(), String> {
    use bzip2::write::BzEncoder;
    use bzip2::Compression;
    use tar::{Builder, Header};

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = File::create(dest).map_err(|e| e.to_string())?;
    let encoder = BzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);
    for (rel, contents, mode) in files {
        let archive_path = format!("{top}/{rel}");
        let mut header = Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(*mode);
        header.set_entry_type(tar::EntryType::Regular);
        {
            let gnu = header
                .as_gnu_mut()
                .ok_or_else(|| "cabecera GNU inválida".to_string())?;
            gnu.name = [0; 100];
            let bytes = archive_path.as_bytes();
            if bytes.len() >= gnu.name.len() {
                return Err(format!("ruta tar demasiado larga: {archive_path}"));
            }
            gnu.name[..bytes.len()].copy_from_slice(bytes);
        }
        header.set_cksum();
        builder
            .append(&header, *contents)
            .map_err(|e| e.to_string())?;
    }
    let encoder = builder.into_inner().map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
pub(crate) const API_HEADER_SNIPPET: &str = r#"#define CEF_API_VERSION_MIN CEF_API_VERSION_13300
#define CEF_API_VERSION_LAST CEF_API_VERSION_15200
"#;

#[cfg(test)]
pub(crate) const VERSION_HEADER_SNIPPET: &str = r#"#define CEF_VERSION "152.0.6+g708dc14+chromium-152.0.7977.83"
#define CHROME_VERSION_MAJOR 152
#define CHROME_VERSION_MINOR 0
#define CHROME_VERSION_BUILD 7977
#define CHROME_VERSION_PATCH 83
"#;

#[cfg(test)]
mod tests {
    use super::super::manifest::{self, REQUIRED_FILES_LINUX64};
    use super::super::paths::PLATFORM;
    use super::*;
    use tempfile::TempDir;

    fn sample_files() -> Vec<(&'static str, &'static [u8], u32)> {
        vec![
            ("Release/libcef.so", b"libcef", 0o644),
            ("Release/chrome-sandbox", b"sandbox", 0o4755),
            ("Release/libEGL.so", b"egl", 0o644),
            ("Resources/resources.pak", b"pak", 0o644),
            ("Resources/locales/en-US.pak", b"en", 0o644),
            (
                "include/cef_api_versions.h",
                API_HEADER_SNIPPET.as_bytes(),
                0o644,
            ),
            (
                "include/cef_version.h",
                VERSION_HEADER_SNIPPET.as_bytes(),
                0o644,
            ),
            ("include/cef_app.h", b"skip-me", 0o644),
            ("LICENSE.txt", b"license", 0o644),
            ("libcef_dll/wrapper.cc", b"dll", 0o644),
            ("cmake/CMakeLists.txt", b"cmake", 0o644),
            ("bazel/BUILD.bazel", b"bazel", 0o644),
        ]
    }

    #[test]
    fn extract_runtime_flattens_and_skips_sdk_dirs() {
        let tmp = TempDir::new().unwrap();
        let tarball = tmp.path().join("mini.tar.bz2");
        write_synthetic_tarball(&tarball, "cef_binary_test_linux64_minimal", &sample_files())
            .unwrap();
        let slot = tmp.path().join("slot");
        let report = extract_runtime(&tarball, &slot).expect("extract");
        assert!(report.files >= 8);
        assert!(slot.join("libcef.so").is_file());
        assert!(slot.join("chrome-sandbox").is_file());
        assert!(slot.join("libEGL.so").is_file());
        assert!(slot.join("resources.pak").is_file());
        assert!(slot.join("locales/en-US.pak").is_file());
        assert!(slot.join("include/cef_api_versions.h").is_file());
        assert!(slot.join("include/cef_version.h").is_file());
        assert!(slot.join("LICENSE.txt").is_file());
        assert!(!slot.join("libcef_dll").exists());
        assert!(!slot.join("cmake").exists());
        assert!(!slot.join("include/cef_app.h").exists());
        assert_eq!(fs::read(slot.join("LICENSE.txt")).unwrap(), b"license");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let sandbox = fs::metadata(slot.join("chrome-sandbox")).unwrap();
            assert_eq!(sandbox.permissions().mode() & 0o777, 0o755);
            let lib = fs::metadata(slot.join("libcef.so")).unwrap();
            assert_eq!(lib.permissions().mode() & 0o777, 0o755);
        }
    }

    #[test]
    fn extract_runtime_rejects_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let tarball = tmp.path().join("evil.tar.bz2");
        write_synthetic_tarball(&tarball, "top", &[("../evil", b"pwned", 0o644)]).unwrap();
        let slot = tmp.path().join("slot");
        let error = extract_runtime(&tarball, &slot).unwrap_err();
        assert!(error.contains(".."), "{error}");
        assert!(!tmp.path().join("evil").exists());
        assert!(!slot.join("evil").exists());
    }

    #[test]
    fn parse_headers_from_real_snippets() {
        let (min, last) = parse_api_versions(API_HEADER_SNIPPET).unwrap();
        assert_eq!(min, 13300);
        assert_eq!(last, 15200);
        let (cef, chromium) = parse_cef_version(VERSION_HEADER_SNIPPET).unwrap();
        assert_eq!(cef, "152.0.6+g708dc14+chromium-152.0.7977.83");
        assert_eq!(chromium, "152.0.7977.83");
    }

    #[test]
    fn build_manifest_walks_sorted_relative_paths() {
        let tmp = TempDir::new().unwrap();
        let slot = tmp.path().join("slot");
        fs::create_dir_all(slot.join("locales")).unwrap();
        fs::write(slot.join("libcef.so"), b"aa").unwrap();
        fs::write(slot.join("locales/en-US.pak"), b"bb").unwrap();
        fs::write(slot.join("manifest.json"), b"{}").unwrap();
        let archive = IndexFile {
            kind: "minimal".into(),
            name: "cef_binary_x_linux64_minimal.tar.bz2".into(),
            sha1: "abc".into(),
            size: 12,
        };
        let manifest = build_manifest(
            &slot,
            "152.0.6+g708dc14+chromium-152.0.7977.83",
            "152.0.7977.83",
            PLATFORM,
            13300,
            15200,
            &archive,
            true,
        )
        .unwrap();
        assert_eq!(manifest.source, SlotSource::Downloaded);
        assert!(!manifest.verified);
        assert_eq!(manifest.archive_name, archive.name);
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(manifest.files[0].path, "libcef.so");
        assert_eq!(manifest.files[1].path, "locales/en-US.pak");
        assert_eq!(manifest.files[0].size, 2);
        assert!(!manifest.created_at.is_empty());
        assert!(!manifest.files.iter().any(|f| f.path == "manifest.json"));
    }

    #[test]
    fn extract_real_tarball_gated() {
        if std::env::var("IDIOTEQUE_CEF_REAL_TARBALL").is_err() {
            return;
        }
        let tarball = Path::new("/tmp/cef152.tar.bz2");
        if !tarball.is_file() {
            return;
        }
        let tmp = TempDir::new().unwrap();
        let slot = tmp.path().join("slot");
        let report = extract_runtime(tarball, &slot).expect("extract real");
        assert!(report.files > 10);
        assert!(slot.join("libcef.so").is_file());
        let api = fs::read_to_string(slot.join("include/cef_api_versions.h")).unwrap();
        let (min, last) = parse_api_versions(&api).unwrap();
        assert_eq!(min, 13300);
        assert_eq!(last, 15200);
        let ver = fs::read_to_string(slot.join("include/cef_version.h")).unwrap();
        let (cef, chromium) = parse_cef_version(&ver).unwrap();
        assert!(cef.starts_with("152.0.6"));
        assert_eq!(chromium, "152.0.7977.83");

        let archive = IndexFile {
            kind: "minimal".into(),
            name: "cef_binary_152.0.6+g708dc14+chromium-152.0.7977.83_linux64_minimal.tar.bz2"
                .into(),
            sha1: "9711b86c105fb590da576fe5a829802f1a79d520".into(),
            size: 321503907,
        };
        let mut manifest =
            build_manifest(&slot, &cef, &chromium, PLATFORM, min, last, &archive, false).unwrap();
        // validate exige los archivos obligatorios; el tarball real los trae.
        for name in REQUIRED_FILES_LINUX64 {
            assert!(
                slot.join(name).is_file(),
                "falta {name} tras extraer el tarball real"
            );
        }
        manifest::save(&slot, &manifest).unwrap();
        manifest = manifest::load(&slot).unwrap();
        manifest::validate(&slot, &manifest).expect("validate");
    }
}
