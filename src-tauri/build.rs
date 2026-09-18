use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let manifest_dir = Path::new(&manifest_dir);
    cef_pin_guard(manifest_dir);
    cef_runtime_guard(manifest_dir);
    tauri_build::build()
}

/// `base.json` y el crate `cef` de Cargo.lock deben ser la misma CEF.
/// Si no, el host (bindings 152) cargaría un `libcef` de otra serie.
fn cef_pin_guard(manifest_dir: &Path) {
    let base_path = manifest_dir.join("cef").join("base.json");
    let lock_path = manifest_dir.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", base_path.display());
    println!("cargo:rerun-if-changed={}", lock_path.display());

    let lock = fs::read_to_string(&lock_path).unwrap_or_default();
    let base = fs::read_to_string(&base_path).unwrap_or_default();
    let Some(crate_ver) = cef_version_from_cargo_lock(&lock) else {
        panic!(
            "No hay una versión única del crate `cef` en {}. \
             El pin de cef/base.json debe coincidir con el sufijo + del crate \
             (152.3.0+152.0.6 → 152.0.6).",
            lock_path.display()
        );
    };
    let Some(base_ver) = json_string_field(&base, "cefVersion") else {
        panic!("{} no tiene cefVersion", base_path.display());
    };
    if !base_matches_crate(&base_ver, &crate_ver) {
        panic!(
            "cef/base.json cefVersion={base_ver} no coincide con el crate cef \
             (+{crate_ver}). Actualiza ambos (docs/CEF-RUNTIME.md)."
        );
    }
    if let Some(chromium) = json_string_field(&base, "chromiumVersion") {
        let marker = format!("+chromium-{chromium}");
        if !base_ver.contains(&marker) {
            panic!("cef/base.json chromiumVersion={chromium} no aparece en cefVersion={base_ver}");
        }
    }
}

/// El paquete lleva Chromium dentro: `bundle.resources` apunta a `cef-base/` y
/// `bundle.externalBin` a `binaries/cef-host-<triple>`. Ambos los genera
/// `bun run cef:prepare` (lo lanzan `bun run tauri dev` y `beforeBuildCommand`).
/// tauri-build ignora en silencio un glob de `resources` sin coincidencias, así
/// que sin esta guardia un release saltándose el pipeline saldría sin navegador.
/// En release es error; en debug solo aviso, para que `cargo test` y
/// `cargo check` no dependan de tener CEF preparado.
fn cef_runtime_guard(manifest_dir: &Path) {
    let target = std::env::var("TARGET").expect("TARGET");
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let base_manifest = manifest_dir.join("cef-base").join("manifest.json");
    let host = manifest_dir
        .join("binaries")
        .join(format!("cef-host-{target}"));
    println!("cargo:rerun-if-changed={}", base_manifest.display());
    println!("cargo:rerun-if-changed={}", host.display());

    let mut missing = Vec::new();
    if !base_manifest.is_file() {
        missing.push(base_manifest.display().to_string());
    }
    if !host.is_file() {
        missing.push(host.display().to_string());
    }
    if missing.is_empty() {
        return;
    }

    let message = format!(
        "Falta el runtime CEF que va dentro del paquete: {}. Ejecuta `bun run cef:prepare` \
         (o `bun run tauri dev` / `bun run tauri build`, que lo hacen solos).",
        missing.join(", ")
    );
    if profile == "release" {
        panic!("{message}");
    }
    println!("cargo:warning={message}");
}

fn cef_version_from_cargo_lock(lock: &str) -> Option<String> {
    let lines: Vec<&str> = lock.lines().collect();
    let mut found = BTreeSet::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() != "name = \"cef\"" {
            continue;
        }
        let Some(version_line) = lines.get(i + 1) else {
            continue;
        };
        let Some(ver) = quoted_field(version_line, "version") else {
            continue;
        };
        if let Some((_, after)) = ver.split_once('+') {
            if !after.is_empty() {
                found.insert(after.to_string());
            }
        }
    }
    if found.len() == 1 {
        found.pop_first()
    } else {
        None
    }
}

fn quoted_field(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = \"");
    line.trim()
        .strip_prefix(&prefix)
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_string)
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let start = text.find(&pat)?;
    let after = &text[start + pat.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn base_matches_crate(base: &str, crate_ver: &str) -> bool {
    !base.is_empty() && !crate_ver.is_empty() && base.starts_with(&format!("{crate_ver}+"))
}
