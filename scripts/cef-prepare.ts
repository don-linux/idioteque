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
import { fileURLToPath } from "node:url";

export const ROOT = fileURLToPath(new URL("..", import.meta.url));
const SRC_TAURI = path.join(ROOT, "src-tauri");
const BASE_JSON_PATH = path.join(SRC_TAURI, "cef", "base.json");
const SDK_ROOT = path.join(SRC_TAURI, ".cef-sdk");
const CEF_BASE_DIR = path.join(SRC_TAURI, "cef-base");
const BINARIES_DIR = path.join(SRC_TAURI, "binaries");

export const TRIPLE_TO_PLATFORM: Record<string, string> = {
  "x86_64-unknown-linux-gnu": "linux64",
  "aarch64-unknown-linux-gnu": "linuxarm64",
};

export const REQUIRED_LINUX64 = [
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

export const KNOWN_PREPARE_FLAGS = ["--skip-host", "--skip-base", "--force"] as const;

export interface BaseFile {
  name: string;
  sha1: string;
  size: number;
}

export interface BaseJson {
  cefVersion: string;
  chromiumVersion: string;
  hostApiVersion: number;
  apiVersionMin: number;
  files: Record<string, BaseFile>;
}

export interface ArchiveJson {
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

export interface PrepareFlags {
  skipHost: boolean;
  skipBase: boolean;
  force: boolean;
  unknown: string[];
}

export interface CefPreparePaths {
  srcTauri: string;
  baseJsonPath: string;
  sdkRoot: string;
  cefBaseDir: string;
  binariesDir: string;
}

export interface PrepareDeps {
  rustcHostTriple: () => string;
  prepareHost: (triple: string, force: boolean) => void;
  prepareBase: (platform: string, force: boolean) => void;
  log: (message: string) => void;
}

export class PrepareError extends Error {
  override readonly name = "PrepareError";
  constructor(message: string) {
    super(message);
  }
}

export function fail(message: string): never {
  throw new PrepareError(message);
}

export function defaultPreparePaths(): CefPreparePaths {
  return {
    srcTauri: SRC_TAURI,
    baseJsonPath: BASE_JSON_PATH,
    sdkRoot: SDK_ROOT,
    cefBaseDir: CEF_BASE_DIR,
    binariesDir: BINARIES_DIR,
  };
}

function resolvePaths(override?: Partial<CefPreparePaths>): CefPreparePaths {
  return { ...defaultPreparePaths(), ...override };
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

export function rustcHostTriple(): string {
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

export function platformOfTriple(triple: string): string | undefined {
  return TRIPLE_TO_PLATFORM[triple];
}

export function majorMinorPatch(cefVersion: string): string {
  const plus = cefVersion.indexOf("+");
  return plus === -1 ? cefVersion : cefVersion.slice(0, plus);
}

/** Hex SHA-1: case-insensitive, trim both sides (CDN JSON is not always lowercase). */
export function sha1Eq(actual: string | undefined | null, expected: string): boolean {
  if (typeof actual !== "string") return false;
  const left = actual.trim();
  const right = expected.trim();
  if (left.length === 0 || right.length === 0) return false;
  return left.toLowerCase() === right.toLowerCase();
}

export function assertArchiveMatches(archive: ArchiveJson, platformFile: BaseFile): void {
  if (archive.name !== platformFile.name || !sha1Eq(archive.sha1, platformFile.sha1)) {
    fail(
      `base.json y la SDK descargada no coinciden\n` +
        `  esperado: ${platformFile.name} ${platformFile.sha1}\n` +
        `  SDK:      ${archive.name ?? "(sin name)"} ${archive.sha1 ?? "(sin sha1)"}`,
    );
  }
}

export function parsePrepareFlags(argv: string[]): PrepareFlags {
  const flags = new Set(argv);
  return {
    skipHost: flags.has("--skip-host"),
    skipBase: flags.has("--skip-base"),
    force: flags.has("--force"),
    unknown: [...flags].filter((flag) => !KNOWN_PREPARE_FLAGS.includes(flag as (typeof KNOWN_PREPARE_FLAGS)[number])),
  };
}

export function findVersionDir(sdkRoot: string, prefix: string): string {
  if (!fs.existsSync(sdkRoot)) {
    fail(`No existe el SDK de CEF en ${sdkRoot}. Compila cef-host una vez para descargarlo.`);
  }
  const exact = path.join(sdkRoot, prefix);
  if (fs.existsSync(exact) && fs.statSync(exact).isDirectory()) return exact;
  const matches = fs
    .readdirSync(sdkRoot)
    .filter((name) => name === prefix || name.startsWith(`${prefix}+`) || name.startsWith(`${prefix}-`))
    .map((name) => path.join(sdkRoot, name))
    .filter((p) => fs.statSync(p).isDirectory())
    .sort();
  if (matches.length === 1) return matches[0];
  if (matches.length > 1) {
    fail(`Varios directorios de SDK coinciden con ${prefix}: ${matches.join(", ")}`);
  }
  fail(`No hay un directorio de SDK cuya versión coincida con ${prefix} bajo ${sdkRoot}`);
}

export function findSdkSlot(versionDir: string): string {
  if (fs.existsSync(path.join(versionDir, "archive.json"))) return versionDir;
  const dirs = fs
    .readdirSync(versionDir)
    .map((name) => path.join(versionDir, name))
    .filter((p) => fs.statSync(p).isDirectory())
    .sort();
  const withArchive = dirs.filter((dir) => fs.existsSync(path.join(dir, "archive.json")));
  if (withArchive.length === 1) return withArchive[0];
  if (withArchive.length === 0) {
    fail(`No se encontró archive.json bajo ${versionDir}`);
  }
  fail(`Varios slots de SDK con archive.json bajo ${versionDir}: ${withArchive.join(", ")}`);
}

export function isTopLevelRuntimeFile(name: string): boolean {
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

export function baseAlreadyPrepared(baseDir: string, cefVersion: string): boolean {
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

export function prepareHost(triple: string, force: boolean, paths?: Partial<CefPreparePaths>): void {
  const resolved = resolvePaths(paths);
  console.log("Compilando cef-host en release…");
  const cargo = run("cargo", ["build", "-p", "cef-host", "--release"], resolved.srcTauri);
  if (cargo.status !== 0) {
    process.exit(cargo.status ?? 1);
  }

  const src = path.join(resolved.srcTauri, "target", "release", "cef-host");
  const dest = path.join(resolved.binariesDir, `cef-host-${triple}`);
  if (!fs.existsSync(src)) {
    fail(`No se encontró el binario compilado en ${src}`);
  }

  fs.mkdirSync(resolved.binariesDir, { recursive: true });
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

export function prepareBase(platform: string, force: boolean, paths?: Partial<CefPreparePaths>): void {
  const resolved = resolvePaths(paths);
  if (!fs.existsSync(resolved.baseJsonPath)) {
    fail(`No existe ${resolved.baseJsonPath}`);
  }
  const base = JSON.parse(fs.readFileSync(resolved.baseJsonPath, "utf8")) as BaseJson;
  const versionPrefix = majorMinorPatch(base.cefVersion);

  if (!force && baseAlreadyPrepared(resolved.cefBaseDir, base.cefVersion)) {
    console.log("base ya preparado");
    return;
  }

  const platformFile = base.files[platform];
  if (!platformFile) {
    fail(`base.json no tiene files[${platform}]`);
  }

  console.log(`Buscando SDK ${versionPrefix} bajo ${path.relative(ROOT, resolved.sdkRoot) || resolved.sdkRoot}…`);
  const versionDir = findVersionDir(resolved.sdkRoot, versionPrefix);
  const sdkDir = findSdkSlot(versionDir);
  console.log(`SDK encontrada: ${path.relative(ROOT, sdkDir)}`);

  const archivePath = path.join(sdkDir, "archive.json");
  const archive = JSON.parse(fs.readFileSync(archivePath, "utf8")) as ArchiveJson;
  assertArchiveMatches(archive, platformFile);
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
  fs.rmSync(resolved.cefBaseDir, { recursive: true, force: true });
  fs.mkdirSync(resolved.cefBaseDir, { recursive: true });

  const topNames = fs.readdirSync(sdkDir);
  for (const name of topNames) {
    const src = path.join(sdkDir, name);
    if (!fs.statSync(src).isFile()) continue;
    if (!isTopLevelRuntimeFile(name)) continue;
    const dest = path.join(resolved.cefBaseDir, name);
    if (name === "libcef.so" && (os.platform() === "linux" || platform.startsWith("linux"))) {
      stripLibcef(src, dest);
    } else {
      copyPreservingMode(src, dest);
    }
  }

  const localesSrc = path.join(sdkDir, "locales");
  if (fs.existsSync(localesSrc) && fs.statSync(localesSrc).isDirectory()) {
    const localesDest = path.join(resolved.cefBaseDir, "locales");
    fs.mkdirSync(localesDest, { recursive: true });
    for (const name of fs.readdirSync(localesSrc)) {
      if (!name.endsWith(".pak")) continue;
      copyPreservingMode(path.join(localesSrc, name), path.join(localesDest, name));
    }
  }

  fs.mkdirSync(path.join(resolved.cefBaseDir, "include"), { recursive: true });
  copyPreservingMode(apiHeaderPath, path.join(resolved.cefBaseDir, "include", "cef_api_versions.h"));
  copyPreservingMode(versionHeaderPath, path.join(resolved.cefBaseDir, "include", "cef_version.h"));

  if (platform === "linux64") {
    const missing = REQUIRED_LINUX64.filter((rel) => !fs.existsSync(path.join(resolved.cefBaseDir, rel)));
    if (missing.length > 0) {
      fail(`Slot base inválido, faltan archivos obligatorios: ${missing.join(", ")}`);
    }
    const sandbox = path.join(resolved.cefBaseDir, "chrome-sandbox");
    const sandboxMode = fs.statSync(sandbox).mode;
    if ((sandboxMode & 0o111) === 0) {
      fail("chrome-sandbox no tiene bit de ejecución");
    }
  }

  console.log("Calculando sha256 y escribiendo manifest.json…");
  const files: ManifestFile[] = [];
  for (const rel of walkRelativeFiles(resolved.cefBaseDir)) {
    if (rel === "manifest.json") continue;
    const full = path.join(resolved.cefBaseDir, rel);
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
  fs.writeFileSync(path.join(resolved.cefBaseDir, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  const libcef = path.join(resolved.cefBaseDir, "libcef.so");
  if (fs.existsSync(libcef)) {
    const mb = (fs.statSync(libcef).size / (1024 * 1024)).toFixed(1);
    console.log(`libcef.so stripeado: ${mb} MB`);
  }
  console.log(`Base lista: ${files.length} archivos en src-tauri/cef-base/`);
}

export function runPrepare(argv: string[], deps?: Partial<PrepareDeps>): void {
  const flags = parsePrepareFlags(argv);
  const resolved: PrepareDeps = {
    rustcHostTriple,
    prepareHost,
    prepareBase: (platform, force) => prepareBase(platform, force),
    log: (message) => console.log(message),
    ...deps,
  };

  for (const flag of flags.unknown) {
    resolved.log(`Aviso: flag desconocido ${flag}`);
  }

  const triple = resolved.rustcHostTriple();
  const platform = platformOfTriple(triple);
  if (!platform) fail(`Triple no soportado: ${triple}`);
  resolved.log(`Plataforma: ${triple} (${platform})`);

  if (!flags.skipHost) resolved.prepareHost(triple, flags.force);
  else resolved.log("Omitiendo host (--skip-host)");

  if (!flags.skipBase) resolved.prepareBase(platform, flags.force);
  else resolved.log("Omitiendo base (--skip-base)");

  resolved.log("Listo.");
}

export function main(argv: string[] = process.argv): void {
  try {
    runPrepare(argv.slice(2));
  } catch (error) {
    if (error instanceof PrepareError) {
      console.error(error.message);
      process.exit(1);
    }
    throw error;
  }
}

const invokedDirectly =
  typeof process.argv[1] === "string" && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) main(process.argv);
