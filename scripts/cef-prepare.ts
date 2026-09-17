/**
 * Prepara el sidecar `cef-host` y el slot base de CEF para empaquetar con Tauri.
 *
 * Uso: bun run cef:prepare [--skip-host] [--skip-base] [--force]
 */
import { spawnSync, type SpawnSyncReturns } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const ROOT = path.resolve(path.dirname(path.resolve(process.argv[1] ?? ".")), "..");
const SRC_TAURI = path.join(ROOT, "src-tauri");
const BASE_JSON_PATH = path.join(SRC_TAURI, "cef", "base.json");
const SDK_ROOT = path.join(SRC_TAURI, ".cef-sdk");
const CEF_BASE_DIR = path.join(SRC_TAURI, "cef-base");
const BINARIES_DIR = path.join(SRC_TAURI, "binaries");

const TRIPLE_TO_PLATFORM: Record<string, string> = {
  "x86_64-unknown-linux-gnu": "linux64",
  "aarch64-unknown-linux-gnu": "linuxarm64",
  "x86_64-pc-windows-msvc": "windows64",
  "x86_64-apple-darwin": "macosx64",
  "aarch64-apple-darwin": "macosarm64",
};

const REQUIRED_LINUX64 = [
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

interface BaseFile {
  name: string;
  sha1: string;
  size: number;
}

interface BaseJson {
  cefVersion: string;
  chromiumVersion: string;
  hostApiVersion: number;
  apiVersionMin: number;
  files: Record<string, BaseFile>;
}

interface ArchiveJson {
  name?: string;
  sha1?: string;
}

interface ManifestFile {
  path: string;
  size: number;
  sha256: string;
}

interface SlotManifest {
  schema: 1;
  cefVersion: string;
  chromiumVersion: string;
  platform: string;
  apiVersionMin: number;
  apiVersionLast: number;
  source: "bundled";
  archiveName: string;
  archiveSha1: string;
  archiveSize: number;
  stripped: true;
  files: ManifestFile[];
  verified: false;
  verifiedAt: null;
  createdAt: string;
}

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

function run(command: string, args: string[], cwd: string): SpawnSyncReturns<Buffer> {
  const result = spawnSync(command, args, { cwd, stdio: "inherit", env: process.env });
  if (result.error) {
    fail(`No se pudo ejecutar ${command}: ${result.error.message}`);
  }
  return result;
}

function commandExists(name: string): boolean {
  const probe = spawnSync(name, ["--version"], { stdio: "ignore" });
  return !probe.error;
}

function rustcHostTriple(): string {
  const result = spawnSync("rustc", ["--print", "host-tuple"], {
    encoding: "utf8",
    env: process.env,
  });
  if (result.error) {
    fail(`No se pudo ejecutar rustc: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail("rustc --print host-tuple falló");
  }
  const triple = (result.stdout ?? "").trim();
  if (!triple) fail("rustc no devolvió el target triple");
  return triple;
}

function majorMinorPatch(cefVersion: string): string {
  const plus = cefVersion.indexOf("+");
  return plus === -1 ? cefVersion : cefVersion.slice(0, plus);
}

function findVersionDir(sdkRoot: string, prefix: string): string {
  if (!fs.existsSync(sdkRoot)) {
    fail(`No existe el SDK de CEF en ${sdkRoot}. Compila cef-host una vez para descargarlo.`);
  }
  const exact = path.join(sdkRoot, prefix);
  if (fs.existsSync(exact) && fs.statSync(exact).isDirectory()) return exact;
  const matches = fs
    .readdirSync(sdkRoot)
    .filter((name) => name === prefix || name.startsWith(`${prefix}+`) || name.startsWith(`${prefix}-`))
    .map((name) => path.join(sdkRoot, name))
    .filter((p) => fs.statSync(p).isDirectory());
  if (matches.length === 1) return matches[0];
  if (matches.length > 1) {
    fail(`Varios directorios de SDK coinciden con ${prefix}: ${matches.join(", ")}`);
  }
  fail(`No hay un directorio de SDK cuya versión coincida con ${prefix} bajo ${sdkRoot}`);
}

function findSdkSlot(versionDir: string): string {
  if (fs.existsSync(path.join(versionDir, "archive.json"))) return versionDir;
  const dirs = fs
    .readdirSync(versionDir)
    .map((name) => path.join(versionDir, name))
    .filter((p) => fs.statSync(p).isDirectory());
  const withArchive = dirs.filter((dir) => fs.existsSync(path.join(dir, "archive.json")));
  if (withArchive.length === 1) return withArchive[0];
  if (withArchive.length === 0) {
    fail(`No se encontró archive.json bajo ${versionDir}`);
  }
  fail(`Varios slots de SDK con archive.json bajo ${versionDir}: ${withArchive.join(", ")}`);
}

function isTopLevelRuntimeFile(name: string): boolean {
  if (name === "archive.json") return false;
  if (name === "chrome-sandbox" || name === "LICENSE.txt") return true;
  if (name === "vk_swiftshader_icd.json") return true;
  return name.endsWith(".so") || name.endsWith(".so.1") || name.endsWith(".pak") || name.endsWith(".dat") || name.endsWith(".bin");
}

function copyPreservingMode(src: string, dest: string): void {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(src, dest);
  fs.chmodSync(dest, fs.statSync(src).mode);
}

function resolveStrip(): string {
  if (commandExists("strip")) return "strip";
  if (commandExists("llvm-strip")) return "llvm-strip";
  fail("No se encontró strip ni llvm-strip");
}

function stripLibcef(src: string, dest: string): void {
  const stripBin = resolveStrip();
  console.log(`Stripeando libcef.so con ${stripBin} --strip-all…`);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  const result = spawnSync(stripBin, ["--strip-all", "-o", dest, src], { stdio: "inherit" });
  if (result.error) fail(`Falló ${stripBin}: ${result.error.message}`);
  if (result.status !== 0) fail(`${stripBin} --strip-all falló (código ${result.status})`);
  if (!fs.existsSync(dest)) fail("strip no produjo libcef.so de destino");
  fs.chmodSync(dest, fs.statSync(src).mode);
}

function sha256File(filePath: string): string {
  const hash = createHash("sha256");
  const fd = fs.openSync(filePath, "r");
  const buf = Buffer.alloc(1024 * 1024);
  try {
    let n = 0;
    while ((n = fs.readSync(fd, buf, 0, buf.length, null)) > 0) {
      hash.update(buf.subarray(0, n));
    }
  } finally {
    fs.closeSync(fd);
  }
  return hash.digest("hex");
}

function walkRelativeFiles(dir: string, prefix = ""): string[] {
  const names = fs.readdirSync(dir).sort();
  const out: string[] = [];
  for (const name of names) {
    const full = path.join(dir, name);
    const rel = prefix ? `${prefix}/${name}` : name;
    const st = fs.statSync(full);
    if (st.isDirectory()) out.push(...walkRelativeFiles(full, rel));
    else out.push(rel);
  }
  return out;
}

function parseApiVersion(header: string, which: "MIN" | "LAST"): number {
  const re = new RegExp(`#define\\s+CEF_API_VERSION_${which}\\s+CEF_API_VERSION_(\\d+)`);
  const match = header.match(re);
  if (!match) fail(`No se encontró CEF_API_VERSION_${which} en include/cef_api_versions.h`);
  return Number(match[1]);
}

function parseDefineString(header: string, name: string): string {
  const match = header.match(new RegExp(`#define\\s+${name}\\s+"([^"]+)"`));
  if (!match) fail(`No se encontró ${name} en include/cef_version.h`);
  return match[1];
}

function parseDefineNumber(header: string, name: string): number {
  const match = header.match(new RegExp(`#define\\s+${name}\\s+(\\d+)`));
  if (!match) fail(`No se encontró ${name} en include/cef_version.h`);
  return Number(match[1]);
}

function baseAlreadyPrepared(baseDir: string, cefVersion: string): boolean {
  const manifestPath = path.join(baseDir, "manifest.json");
  if (!fs.existsSync(manifestPath)) return false;
  try {
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as {
      cefVersion?: string;
      files?: ManifestFile[];
    };
    if (manifest.cefVersion !== cefVersion || !Array.isArray(manifest.files)) return false;
    for (const file of manifest.files) {
      if (typeof file.path !== "string" || typeof file.size !== "number") return false;
      const full = path.join(baseDir, file.path);
      if (!fs.existsSync(full)) return false;
      if (fs.statSync(full).size !== file.size) return false;
    }
    return true;
  } catch {
    return false;
  }
}

function prepareHost(triple: string, force: boolean): void {
  console.log("Compilando cef-host en release…");
  const cargo = run("cargo", ["build", "-p", "cef-host", "--release"], SRC_TAURI);
  if (cargo.status !== 0) {
    process.exit(cargo.status ?? 1);
  }

  const exe = triple.includes("windows") ? ".exe" : "";
  const src = path.join(SRC_TAURI, "target", "release", `cef-host${exe}`);
  const dest = path.join(BINARIES_DIR, `cef-host-${triple}${exe}`);
  if (!fs.existsSync(src)) {
    fail(`No se encontró el binario compilado en ${src}`);
  }

  fs.mkdirSync(BINARIES_DIR, { recursive: true });
  if (!force && fs.existsSync(dest)) {
    const srcMtime = fs.statSync(src).mtimeMs;
    const destMtime = fs.statSync(dest).mtimeMs;
    if (destMtime >= srcMtime) {
      console.log(`Host ya actualizado en ${path.relative(ROOT, dest)}, no se copia`);
      return;
    }
  }

  console.log(`Copiando cef-host → ${path.relative(ROOT, dest)}`);
  copyPreservingMode(src, dest);
}

function prepareBase(platform: string, force: boolean): void {
  if (!fs.existsSync(BASE_JSON_PATH)) {
    fail(`No existe ${BASE_JSON_PATH}`);
  }
  const base = JSON.parse(fs.readFileSync(BASE_JSON_PATH, "utf8")) as BaseJson;
  const versionPrefix = majorMinorPatch(base.cefVersion);

  if (!force && baseAlreadyPrepared(CEF_BASE_DIR, base.cefVersion)) {
    console.log("base ya preparado");
    return;
  }

  const platformFile = base.files[platform];
  if (!platformFile) {
    fail(`base.json no tiene files[${platform}]`);
  }

  console.log(`Buscando SDK ${versionPrefix} bajo src-tauri/.cef-sdk…`);
  const versionDir = findVersionDir(SDK_ROOT, versionPrefix);
  const sdkDir = findSdkSlot(versionDir);
  console.log(`SDK encontrada: ${path.relative(ROOT, sdkDir)}`);

  const archivePath = path.join(sdkDir, "archive.json");
  const archive = JSON.parse(fs.readFileSync(archivePath, "utf8")) as ArchiveJson;
  if (archive.name !== platformFile.name || archive.sha1 !== platformFile.sha1) {
    fail(
      `base.json y la SDK descargada no coinciden\n` +
        `  esperado: ${platformFile.name} ${platformFile.sha1}\n` +
        `  SDK:      ${archive.name ?? "(sin name)"} ${archive.sha1 ?? "(sin sha1)"}`,
    );
  }
  console.log("base.json y la SDK coinciden.");

  const apiHeaderPath = path.join(sdkDir, "include", "cef_api_versions.h");
  const versionHeaderPath = path.join(sdkDir, "include", "cef_version.h");
  if (!fs.existsSync(apiHeaderPath) || !fs.existsSync(versionHeaderPath)) {
    fail("Faltan include/cef_api_versions.h o include/cef_version.h en la SDK");
  }
  const apiHeader = fs.readFileSync(apiHeaderPath, "utf8");
  const versionHeader = fs.readFileSync(versionHeaderPath, "utf8");
  const apiVersionMin = parseApiVersion(apiHeader, "MIN");
  const apiVersionLast = parseApiVersion(apiHeader, "LAST");
  const cefVersion = parseDefineString(versionHeader, "CEF_VERSION");
  const chromiumVersion = [
    parseDefineNumber(versionHeader, "CHROME_VERSION_MAJOR"),
    parseDefineNumber(versionHeader, "CHROME_VERSION_MINOR"),
    parseDefineNumber(versionHeader, "CHROME_VERSION_BUILD"),
    parseDefineNumber(versionHeader, "CHROME_VERSION_PATCH"),
  ].join(".");

  if (
    cefVersion !== base.cefVersion ||
    chromiumVersion !== base.chromiumVersion ||
    apiVersionMin !== base.apiVersionMin ||
    apiVersionLast !== base.hostApiVersion
  ) {
    fail(
      `Las cabeceras de la SDK no coinciden con base.json\n` +
        `  CEF_VERSION=${cefVersion} (base ${base.cefVersion})\n` +
        `  Chromium=${chromiumVersion} (base ${base.chromiumVersion})\n` +
        `  API min/last=${apiVersionMin}/${apiVersionLast} (base ${base.apiVersionMin}/${base.hostApiVersion})`,
    );
  }

  console.log("Reconstruyendo src-tauri/cef-base/ desde cero…");
  fs.rmSync(CEF_BASE_DIR, { recursive: true, force: true });
  fs.mkdirSync(CEF_BASE_DIR, { recursive: true });

  const topNames = fs.readdirSync(sdkDir);
  for (const name of topNames) {
    const src = path.join(sdkDir, name);
    if (!fs.statSync(src).isFile()) continue;
    if (!isTopLevelRuntimeFile(name)) continue;
    const dest = path.join(CEF_BASE_DIR, name);
    if (name === "libcef.so" && (os.platform() === "linux" || platform.startsWith("linux"))) {
      stripLibcef(src, dest);
    } else {
      copyPreservingMode(src, dest);
    }
  }

  const localesSrc = path.join(sdkDir, "locales");
  if (fs.existsSync(localesSrc) && fs.statSync(localesSrc).isDirectory()) {
    const localesDest = path.join(CEF_BASE_DIR, "locales");
    fs.mkdirSync(localesDest, { recursive: true });
    for (const name of fs.readdirSync(localesSrc)) {
      if (!name.endsWith(".pak")) continue;
      copyPreservingMode(path.join(localesSrc, name), path.join(localesDest, name));
    }
  }

  fs.mkdirSync(path.join(CEF_BASE_DIR, "include"), { recursive: true });
  copyPreservingMode(apiHeaderPath, path.join(CEF_BASE_DIR, "include", "cef_api_versions.h"));
  copyPreservingMode(versionHeaderPath, path.join(CEF_BASE_DIR, "include", "cef_version.h"));

  if (platform === "linux64") {
    const missing = REQUIRED_LINUX64.filter((rel) => !fs.existsSync(path.join(CEF_BASE_DIR, rel)));
    if (missing.length > 0) {
      fail(`Slot base inválido, faltan archivos obligatorios: ${missing.join(", ")}`);
    }
    const sandbox = path.join(CEF_BASE_DIR, "chrome-sandbox");
    const sandboxMode = fs.statSync(sandbox).mode;
    if ((sandboxMode & 0o111) === 0) {
      fail("chrome-sandbox no tiene bit de ejecución");
    }
  }

  console.log("Calculando sha256 y escribiendo manifest.json…");
  const files: ManifestFile[] = [];
  for (const rel of walkRelativeFiles(CEF_BASE_DIR)) {
    if (rel === "manifest.json") continue;
    const full = path.join(CEF_BASE_DIR, rel);
    files.push({
      path: rel.replaceAll(path.sep, "/"),
      size: fs.statSync(full).size,
      sha256: sha256File(full),
    });
  }
  files.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));

  const manifest: SlotManifest = {
    schema: 1,
    cefVersion: base.cefVersion,
    chromiumVersion: base.chromiumVersion,
    platform,
    apiVersionMin,
    apiVersionLast,
    source: "bundled",
    archiveName: platformFile.name,
    archiveSha1: platformFile.sha1,
    archiveSize: platformFile.size,
    stripped: true,
    files,
    verified: false,
    verifiedAt: null,
    createdAt: new Date().toISOString(),
  };
  fs.writeFileSync(path.join(CEF_BASE_DIR, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  const libcef = path.join(CEF_BASE_DIR, "libcef.so");
  if (fs.existsSync(libcef)) {
    const mb = (fs.statSync(libcef).size / (1024 * 1024)).toFixed(1);
    console.log(`libcef.so stripeado: ${mb} MB`);
  }
  console.log(`Base lista: ${files.length} archivos en src-tauri/cef-base/`);
}

function main(): void {
  const flags = new Set(process.argv.slice(2));
  const skipHost = flags.has("--skip-host");
  const skipBase = flags.has("--skip-base");
  const force = flags.has("--force");
  for (const flag of flags) {
    if (flag !== "--skip-host" && flag !== "--skip-base" && flag !== "--force") {
      console.log(`Aviso: flag desconocido ${flag}`);
    }
  }

  const triple = rustcHostTriple();
  const platform = TRIPLE_TO_PLATFORM[triple];
  if (!platform) fail(`Triple no soportado: ${triple}`);
  console.log(`Plataforma: ${triple} (${platform})`);

  if (!skipHost) prepareHost(triple, force);
  else console.log("Omitiendo host (--skip-host)");

  if (!skipBase) prepareBase(platform, force);
  else console.log("Omitiendo base (--skip-base)");

  console.log("Listo.");
}

main();
