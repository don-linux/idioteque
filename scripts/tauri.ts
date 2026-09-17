/**
 * `bun run tauri <subcomando>`: envoltorio del CLI de Tauri.
 *
 * - `dev`: corre `cef:prepare` (sidecar `cef-host` + base de CEF) ANTES de
 *   arrancar el CLI. El CLI espera al dev server como mucho 180 s; si
 *   `cef:prepare` fuera dentro de `beforeDevCommand`, la primera descarga y
 *   compilación de CEF agotaría ese plazo y la app nunca arrancaría.
 * - `build`: `tauri build` (deb y rpm, con CEF vía `resources`/`externalBin`)
 *   y después la AppImage en dos fases: `tauri bundle --bundles appimage` sin
 *   CEF (linuxdeploy hace `ldd` y `patchelf` de todo lo que encuentra en
 *   `usr/bin` y `usr/lib`; fallaría con `libcef.so => not found` y cambiaría
 *   el tamaño de las libs del base) y luego inyección de `cef-base/` y
 *   `cef-host` en el AppDir y reempaquetado con linuxdeploy-plugin-appimage,
 *   que solo construye el squashfs. Toda la fase corre con
 *   `TMPDIR=src-tauri/target/tmp` (salvo que el usuario traiga el suyo): el
 *   staging del bundler y la extracción del plugin no pasan por `/tmp`.
 * - Otros subcomandos: passthrough.
 */
import { spawnSync, type SpawnSyncReturns } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = fileURLToPath(new URL("..", import.meta.url));
const SRC_TAURI = path.join(ROOT, "src-tauri");
const TAURI_BIN = path.join(ROOT, "node_modules", ".bin", "tauri");
const CEF_BASE_DIR = path.join(SRC_TAURI, "cef-base");
const BINARIES_DIR = path.join(SRC_TAURI, "binaries");

const APPIMAGE_PLUGIN_URL =
  "https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-{arch}.AppImage";

/** Config que se pasa a `tauri bundle` para que la AppImage salga sin CEF. */
export const APPIMAGE_OVERRIDE_CONFIG = JSON.stringify({
  bundle: { resources: [], externalBin: [] },
});

export interface BuildPlan {
  /** Args para `tauri build` (con `appimage` quitado de `--bundles` si venía). */
  buildArgs: string[];
  /** Si hay que construir la AppImage con la fase especial. */
  appimage: boolean;
  debug: boolean;
  target: string | undefined;
}

export interface ArtifactNames {
  appDirName: string;
  appImageName: string;
  resourceDirName: string;
}

/** Primer argumento que no es un flag: el subcomando del CLI. */
export function subcommandOf(args: string[]): string | undefined {
  return args.find((arg) => !arg.startsWith("-"));
}

/**
 * Valor de `--bundles` / `-b` (clap: `num_args(0..)`, separador `,`).
 * Devuelve `null` si no se pasó. `span` son los índices ocupados por el flag y
 * sus valores, para poder quitarlos.
 */
export function extractBundles(args: string[]): { bundles: string[]; span: number[] } | null {
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--") return null;
    const inline = arg.match(/^(?:--bundles|-b)=(.*)$/);
    if (inline) {
      return { bundles: splitBundles([inline[1]]), span: [i] };
    }
    if (arg === "--bundles" || arg === "-b") {
      const span = [i];
      const values: string[] = [];
      for (let j = i + 1; j < args.length && !args[j].startsWith("-"); j++) {
        values.push(args[j]);
        span.push(j);
      }
      return { bundles: splitBundles(values), span };
    }
  }
  return null;
}

function splitBundles(values: string[]): string[] {
  return values
    .flatMap((value) => value.split(","))
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
}

/** Valor de `--target` / `-t`, si se pasó. */
export function extractTarget(args: string[]): string | undefined {
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--") return undefined;
    const inline = arg.match(/^(?:--target|-t)=(.*)$/);
    if (inline) return inline[1] || undefined;
    if ((arg === "--target" || arg === "-t") && i + 1 < args.length) return args[i + 1];
  }
  return undefined;
}

export function hasFlag(args: string[], ...flags: string[]): boolean {
  for (const arg of args) {
    if (arg === "--") return false;
    if (flags.includes(arg)) return true;
  }
  return false;
}

/**
 * Decide qué hace `bun run tauri build`:
 * - sin `--bundles`: `tauri build` tal cual (targets de tauri.conf.json) y
 *   además la AppImage (solo en Linux);
 * - con `--bundles` que incluye `appimage`: se quita de la lista (o se pasa
 *   `--no-bundle` si era el único) y la AppImage va por la fase especial;
 * - con `--bundles` sin `appimage`, `--no-bundle` o `--help`: passthrough.
 */
export function planBuild(args: string[], platform: NodeJS.Platform = process.platform): BuildPlan {
  const debug = hasFlag(args, "--debug", "-d");
  const target = extractTarget(args);
  const passthrough: BuildPlan = { buildArgs: args, appimage: false, debug, target };

  if (platform !== "linux") return passthrough;
  if (hasFlag(args, "--no-bundle", "--help", "-h")) return passthrough;

  const requested = extractBundles(args);
  if (requested === null) {
    return { buildArgs: args, appimage: true, debug, target };
  }
  if (!requested.bundles.includes("appimage")) return passthrough;

  const others = requested.bundles.filter((bundle) => bundle !== "appimage");
  const span = new Set(requested.span);
  const buildArgs = args.filter((_, index) => !span.has(index));
  if (others.length === 0) {
    buildArgs.push("--no-bundle");
  } else {
    buildArgs.push("--bundles", others.join(","));
  }
  return { buildArgs, appimage: true, debug, target };
}

/** Arquitectura como la nombra Tauri en el fichero AppImage. */
export function appImageArch(rustArch: string): string {
  switch (rustArch) {
    case "x86_64":
      return "amd64";
    case "i686":
      return "i386";
    case "aarch64":
      return "aarch64";
    case "armv7":
      return "armhf";
    default:
      throw new Error(`Arquitectura sin AppImage en Tauri: ${rustArch}`);
  }
}

/** Arquitectura como la usan linuxdeploy y sus plugins (`ARCH`). */
export function toolsArch(rustArch: string): string {
  return rustArch === "armv7" ? "armhf" : rustArch;
}

export function archOfTriple(triple: string): string {
  const arch = triple.split("-")[0];
  if (!arch) throw new Error(`Triple inválido: ${triple}`);
  return arch;
}

export function artifactNames(
  conf: { productName: string; version: string },
  rustArch: string,
): ArtifactNames {
  return {
    appDirName: `${conf.productName}.AppDir`,
    appImageName: `${conf.productName}_${conf.version}_${appImageArch(rustArch)}.AppImage`,
    resourceDirName: conf.productName,
  };
}

/** `src-tauri/target[/<triple>]/<release|debug>` (cargo separa por target). */
export function profileDir(target: string | undefined, debug: boolean): string {
  const parts = [SRC_TAURI, "target"];
  if (target) parts.push(target);
  parts.push(debug ? "debug" : "release");
  return path.join(...parts);
}

/** `$XDG_CACHE_HOME/tauri` o `~/.cache/tauri`: donde el CLI cachea linuxdeploy y sus plugins. */
export function tauriToolsDir(env: NodeJS.ProcessEnv = process.env, home: string = os.homedir()): string {
  const cache = env.XDG_CACHE_HOME && env.XDG_CACHE_HOME.length > 0 ? env.XDG_CACHE_HOME : path.join(home, ".cache");
  return path.join(cache, "tauri");
}

/** `src-tauri/target/tmp`: temporales del build en disco, no en el tmpfs de `/tmp`. */
export function buildTmpDir(): string {
  return path.join(SRC_TAURI, "target", "tmp");
}

/**
 * Entorno para la fase de build. `tauri-bundler` monta el deb/rpm en
 * `tempdir()` y el plugin de AppImage se extrae en `$TMPDIR`; con `/tmp` en
 * un tmpfs con cuota por usuario (systemd ≥ 258) un bundle de ~350 MB con CEF
 * acaba en `Disk quota exceeded (os error 122)`. Si el usuario ya trae
 * `TMPDIR`, se respeta.
 */
export function buildEnv(env: NodeJS.ProcessEnv, tmpDir: string): NodeJS.ProcessEnv {
  if (env.TMPDIR && env.TMPDIR.length > 0) return { ...env };
  return { ...env, TMPDIR: tmpDir };
}

// ---------------------------------------------------------------------------
// Ejecución
// ---------------------------------------------------------------------------

function fail(message: string): never {
  console.error(`[tauri] ${message}`);
  process.exit(1);
}

function exitWith(result: SpawnSyncReturns<Buffer>, what: string): void {
  if (result.error) fail(`No se pudo ejecutar ${what}: ${result.error.message}`);
  if (result.status !== 0) {
    if (result.signal) fail(`${what} terminó por señal ${result.signal}`);
    process.exit(result.status ?? 1);
  }
}

function runTauri(args: string[], env: NodeJS.ProcessEnv = process.env): SpawnSyncReturns<Buffer> {
  return spawnSync(TAURI_BIN, args, { cwd: ROOT, stdio: "inherit", env });
}

function runCefPrepare(): void {
  console.log("[tauri] Preparando CEF (cef-host + base) antes de arrancar el CLI…");
  const result = spawnSync(process.execPath, ["run", "cef:prepare"], {
    cwd: ROOT,
    stdio: "inherit",
    env: process.env,
  });
  exitWith(result, "bun run cef:prepare");
}

function rustcHostTriple(): string {
  const result = spawnSync("rustc", ["--print", "host-tuple"], { encoding: "utf8", env: process.env });
  if (result.error) fail(`No se pudo ejecutar rustc: ${result.error.message}`);
  if (result.status !== 0) fail("rustc --print host-tuple falló");
  const triple = (result.stdout ?? "").trim();
  if (!triple) fail("rustc no devolvió el target triple");
  return triple;
}

function readTauriConf(): { productName: string; version: string } {
  const confPath = path.join(SRC_TAURI, "tauri.conf.json");
  const conf = JSON.parse(fs.readFileSync(confPath, "utf8")) as { productName?: string; version?: string };
  if (!conf.productName || !conf.version) fail(`${confPath} necesita productName y version`);
  return { productName: conf.productName, version: conf.version };
}

function copyPreservingMode(src: string, dest: string): void {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(src, dest);
  fs.chmodSync(dest, fs.statSync(src).mode);
}

function copyTree(srcDir: string, destDir: string): void {
  for (const entry of fs.readdirSync(srcDir, { withFileTypes: true })) {
    const src = path.join(srcDir, entry.name);
    const dest = path.join(destDir, entry.name);
    if (entry.isDirectory()) copyTree(src, dest);
    else if (entry.isFile()) copyPreservingMode(src, dest);
  }
}

function humanMb(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

interface ManifestFile {
  path: string;
  size: number;
}

/** Inyecta el base y el sidecar en el AppDir y comprueba que quedan intactos. */
function injectCefIntoAppDir(appDir: string, resourceDirName: string, triple: string): void {
  const manifestPath = path.join(CEF_BASE_DIR, "manifest.json");
  if (!fs.existsSync(manifestPath)) {
    fail(`No existe ${path.relative(ROOT, manifestPath)}; ejecuta \`bun run cef:prepare\``);
  }
  const hostSrc = path.join(BINARIES_DIR, `cef-host-${triple}`);
  if (!fs.existsSync(hostSrc)) {
    fail(`No existe ${path.relative(ROOT, hostSrc)}; ejecuta \`bun run cef:prepare\``);
  }

  const baseDest = path.join(appDir, "usr", "lib", resourceDirName, "cef", "base");
  const hostDest = path.join(appDir, "usr", "bin", "cef-host");
  fs.rmSync(path.dirname(baseDest), { recursive: true, force: true });
  fs.rmSync(hostDest, { force: true });

  console.log(`[tauri] Inyectando cef-base → ${path.relative(appDir, baseDest)}`);
  copyTree(CEF_BASE_DIR, baseDest);
  console.log(`[tauri] Inyectando cef-host → ${path.relative(appDir, hostDest)}`);
  copyPreservingMode(hostSrc, hostDest);
  fs.chmodSync(hostDest, 0o755);

  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as { files?: ManifestFile[] };
  for (const file of manifest.files ?? []) {
    const full = path.join(baseDest, file.path);
    if (!fs.existsSync(full)) fail(`Falta ${file.path} en el AppDir tras la inyección`);
    const size = fs.statSync(full).size;
    if (size !== file.size) {
      fail(`Tamaño de ${file.path} en el AppDir (${size}) no coincide con el manifest (${file.size})`);
    }
  }
  const sandbox = path.join(baseDest, "chrome-sandbox");
  if (fs.existsSync(sandbox) && (fs.statSync(sandbox).mode & 0o111) === 0) {
    fail("chrome-sandbox perdió el bit de ejecución en el AppDir");
  }
}

function resolveAppImagePlugin(arch: string): string {
  const override = process.env.IDIOTEQUE_APPIMAGE_PLUGIN;
  if (override) {
    if (!fs.existsSync(override)) fail(`IDIOTEQUE_APPIMAGE_PLUGIN apunta a ${override}, que no existe`);
    return override;
  }
  const toolsDir = tauriToolsDir();
  const plugin = path.join(toolsDir, "linuxdeploy-plugin-appimage.AppImage");
  if (fs.existsSync(plugin)) return plugin;

  const url = APPIMAGE_PLUGIN_URL.replace("{arch}", arch);
  console.log(`[tauri] Descargando linuxdeploy-plugin-appimage desde ${url}`);
  fs.mkdirSync(toolsDir, { recursive: true });
  const download = spawnSync("curl", ["-fL", "--retry", "3", "-o", plugin, url], {
    stdio: "inherit",
    env: process.env,
  });
  if (download.error || download.status !== 0) {
    fs.rmSync(plugin, { force: true });
    fail("No se pudo descargar linuxdeploy-plugin-appimage (o instala curl, o fija IDIOTEQUE_APPIMAGE_PLUGIN)");
  }
  fs.chmodSync(plugin, 0o770);
  return plugin;
}

/** Fase AppImage: `tauri bundle` sin CEF, inyección y reempaquetado. */
function buildAppImage(plan: BuildPlan, env: NodeJS.ProcessEnv): void {
  const triple = plan.target ?? rustcHostTriple();
  const rustArch = archOfTriple(triple);
  const conf = readTauriConf();
  const names = artifactNames(conf, rustArch);
  const bundleDir = path.join(profileDir(plan.target, plan.debug), "bundle", "appimage");
  const appDir = path.join(bundleDir, names.appDirName);
  const appImage = path.join(bundleDir, names.appImageName);

  console.log("[tauri] Fase AppImage 1/3: tauri bundle sin CEF (linuxdeploy no debe tocar libcef)");
  const bundleArgs = ["bundle", "--bundles", "appimage", "--config", APPIMAGE_OVERRIDE_CONFIG];
  if (plan.debug) bundleArgs.push("--debug");
  if (plan.target) bundleArgs.push("--target", plan.target);
  exitWith(runTauri(bundleArgs, env), "tauri bundle --bundles appimage");

  if (!fs.existsSync(appDir)) fail(`tauri bundle no dejó ${path.relative(ROOT, appDir)}`);
  if (!fs.existsSync(appImage)) fail(`tauri bundle no produjo ${path.relative(ROOT, appImage)}`);

  console.log("[tauri] Fase AppImage 2/3: inyectar el runtime CEF en el AppDir");
  injectCefIntoAppDir(appDir, names.resourceDirName, triple);

  console.log("[tauri] Fase AppImage 3/3: reempaquetar el AppDir");
  const arch = toolsArch(rustArch);
  const plugin = resolveAppImagePlugin(arch);
  fs.rmSync(appImage, { force: true });
  const pack = spawnSync(plugin, ["--appimage-extract-and-run", `--appdir=${appDir}`], {
    cwd: bundleDir,
    stdio: "inherit",
    env: {
      ...env,
      OUTPUT: appImage,
      ARCH: arch,
      APPIMAGE_EXTRACT_AND_RUN: "1",
    },
  });
  exitWith(pack, "linuxdeploy-plugin-appimage");
  if (!fs.existsSync(appImage)) fail(`El plugin no produjo ${path.relative(ROOT, appImage)}`);

  console.log(`[tauri] AppImage con CEF: ${path.relative(ROOT, appImage)} (${humanMb(fs.statSync(appImage).size)})`);
}

function main(argv: string[]): void {
  const args = argv.slice(2);
  const subcommand = subcommandOf(args);

  // Con Ctrl+C el CLI hijo se encarga (mata beforeDevCommand y sale); el
  // wrapper solo espera y propaga su código de salida.
  process.on("SIGINT", () => {});
  process.on("SIGTERM", () => {});

  if (subcommand === "dev") {
    runCefPrepare();
    exitWith(runTauri(args), "tauri dev");
    return;
  }

  if (subcommand === "build") {
    const plan = planBuild(args);
    const tmpDir = buildTmpDir();
    fs.mkdirSync(tmpDir, { recursive: true });
    const env = buildEnv(process.env, tmpDir);
    exitWith(runTauri(plan.buildArgs, env), "tauri build");
    if (plan.appimage) buildAppImage(plan, env);
    return;
  }

  exitWith(runTauri(args), `tauri ${subcommand ?? ""}`.trim());
}

const invokedDirectly =
  typeof process.argv[1] === "string" && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) main(process.argv);
