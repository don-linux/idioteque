use std::path::Path;

fn main() {
    cef_runtime_guard();
    tauri_build::build()
}

/// El paquete lleva Chromium dentro: `bundle.resources` apunta a `cef-base/` y
/// `bundle.externalBin` a `binaries/cef-host-<triple>`. Ambos los genera
/// `bun run cef:prepare` (lo lanzan `bun run tauri dev` y `beforeBuildCommand`).
/// tauri-build ignora en silencio un glob de `resources` sin coincidencias, así
/// que sin esta guardia un release saltándose el pipeline saldría sin navegador.
/// En release es error; en debug solo aviso, para que `cargo test` y
/// `cargo check` no dependan de tener CEF preparado.
fn cef_runtime_guard() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let target = std::env::var("TARGET").expect("TARGET");
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let exe = if target.contains("windows") { ".exe" } else { "" };

    let base_manifest = Path::new(&manifest_dir).join("cef-base").join("manifest.json");
    let host = Path::new(&manifest_dir)
        .join("binaries")
        .join(format!("cef-host-{target}{exe}"));
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
