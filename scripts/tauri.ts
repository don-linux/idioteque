/**
 * `bun run tauri <subcomando>`: envoltorio del CLI de Tauri.
 *
 * - `dev`: corre `cef:prepare` (sidecar `cef-host` + base de CEF) ANTES de
 *   arrancar el CLI. El CLI espera al dev server como mucho 180 s; si
 *   `cef:prepare` fuera dentro de `beforeDevCommand`, la primera descarga y
 *   compilación de CEF agotaría ese plazo y la app nunca arrancaría.
 * - `build`: `tauri build` (deb y rpm, con CEF vía `resources`/`externalBin`)
 *   y después la AppImage en tres fases: `tauri bundle --bundles appimage` sin
 *   CEF (linuxdeploy hace `ldd` y `patchelf` de todo lo que encuentra en
 *   `usr/bin` y `usr/lib`; fallaría con `libcef.so => not found` y cambiaría
 *   el tamaño de las libs del base), **inyección DESPUÉS de linuxdeploy** de
 *   `cef-base/` y `cef-host` en el AppDir, y reempaquetado con
 *   linuxdeploy-plugin-appimage (solo squashfs). Las tres superficies Linux
 *   (deb, rpm, AppImage) corren con `TMPDIR=src-tauri/target/tmp` salvo que
 *   el usuario traiga el suyo: el staging del bundler y la extracción del
 *   plugin no pasan por `/tmp`. Eso no es un quirk de Ubuntu: `/tmp` tmpfs
 *   con usrquota (systemd ≥ 258) aparece en Debian, Fedora/RHEL y openSUSE.
 *   El rpm va sin compresión (`none`): gzip de ~350 MB tarda decenas de
 *   minutos. Si quitar el TMPDIR de build o la inyección post-linuxdeploy
 *   rompe el empaquetado, se conservan (política de workarounds).
 * - `dev`: `cef:prepare` debe terminar bien; un fallo no lanza `tauri dev`.
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
  if (rustArch === "x86_64") return "amd64";
  throw new Error(`Arquitectura sin AppImage en Tauri: ${rustArch}`);
}

/** Arquitectura como la usan linuxdeploy y sus plugins (`ARCH`). */
export function toolsArch(rustArch: string): string {
  return rustArch;
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

/** Superficies Linux reales del producto. Ubuntu cloud es un lab, no “Linux”. */
export const LINUX_PACKAGE_SURFACES = ["deb", "rpm", "appimage"] as const;
export type LinuxPackageSurface = (typeof LINUX_PACKAGE_SURFACES)[number];

/** Orden fijo de la AppImage: linuxdeploy sin CEF → inyectar → squashfs. */
export const APPIMAGE_STEPS = ["linuxdeploy", "inject-cef", "repack"] as const;
export type AppImageStep = (typeof APPIMAGE_STEPS)[number];

export type DevStep = "cef-prepare" | "tauri-dev";

export const RPM_COMPRESSION_NONE = "none" as const;

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
 *
 * Workaround conservado: no se vuelve a `/tmp` “porque el doc no lo pide”.
 * El staging grande (rpm `compression: none`, AppImage + CEF) no cabe en
 * tmpfs con usrquota en Debian/Fedora/RHEL/openSUSE, no solo en Ubuntu.
 */
export function buildEnv(env: NodeJS.ProcessEnv, tmpDir: string): NodeJS.ProcessEnv {
  if (env.TMPDIR && env.TMPDIR.length > 0) return { ...env };
  return { ...env, TMPDIR: tmpDir };
}

/**
 * TMPDIR de staging para **cada** superficie. No hay camino AppImage→`/tmp`
 * ni rpm→`/var/tmp`: las tres usan `src-tauri/target/tmp`.
 */
export function envForLinuxSurface(
  surface: LinuxPackageSurface,
  env: NodeJS.ProcessEnv,
  tmpDir: string,
): NodeJS.ProcessEnv {
  if (!LINUX_PACKAGE_SURFACES.includes(surface)) {
    throw new Error(`Superficie Linux desconocida: ${String(surface)}`);
  }
  return buildEnv(env, tmpDir);
}

/** tmpfs / cuota del sistema: no son el default del wrapper en ninguna distro. */
export function isSystemTmpDir(dir: string): boolean {
  const normalized = path.resolve(dir);
  return (
    normalized === "/tmp" ||
    normalized === "/var/tmp" ||
    normalized === "/dev/shm" ||
    normalized.startsWith("/run/user/")
  );
}

/**
 * Qué artefactos Linux va a tocar esta invocación.
 * Sin `--bundles`, Tauri empaqueta deb+rpm (`tauri.conf.json`) y el wrapper
 * añade la AppImage. `--bundles rpm` es Fedora/RHEL/openSUSE, no Ubuntu.
 */
export function linuxSurfacesInPlay(
  args: string[],
  platform: NodeJS.Platform = "linux",
): LinuxPackageSurface[] {
  if (hasFlag(args, "--help", "-h")) return [];
  const plan = planBuild(args, platform);
  if (hasFlag(args, "--no-bundle")) {
    return plan.appimage ? ["appimage"] : [];
  }
  const requested = extractBundles(args);
  const surfaces: LinuxPackageSurface[] = [];
  if (requested === null) {
    surfaces.push("deb", "rpm");
  } else {
    if (requested.bundles.includes("deb")) surfaces.push("deb");
    if (requested.bundles.includes("rpm")) surfaces.push("rpm");
  }
  if (plan.appimage) surfaces.push("appimage");
  return surfaces;
}

export function nextAppImageStep(completed: readonly AppImageStep[]): AppImageStep | "done" {
  for (const step of APPIMAGE_STEPS) {
    if (!completed.includes(step)) return step;
  }
  return "done";
}

/** Inyectar CEF solo cuando linuxdeploy ya terminó y aún no se reempaquetó. */
export function canInjectCefAfterLinuxdeploy(completed: readonly AppImageStep[]): boolean {
  return nextAppImageStep(completed) === "inject-cef";
}

/**
 * `dev` no lanza el CLI si `cef:prepare` falló (status, señal o ENOENT).
 * Saltar prepare rompe el arranque (sidecar / base ausentes).
 */
export function nextDevStep(
  completed: readonly DevStep[],
  prepare: { ok: boolean } | undefined,
): DevStep | "done" | "abort" {
  if (!completed.includes("cef-prepare")) return "cef-prepare";
  if (!prepare?.ok) return "abort";
  if (!completed.includes("tauri-dev")) return "tauri-dev";
  return "done";
}

export function cefPrepareArgv(execPath: string): { command: string; args: readonly string[] } {
  return { command: execPath, args: ["run", "cef:prepare"] };
}

export type ChildExit = { ok: true } | { ok: false; exitCode: number; message: string };

export function interpretChildExit(
  result: { error?: Error | null; status: number | null; signal: NodeJS.Signals | null },
  what: string,
): ChildExit {
  if (result.error) {
    return { ok: false, exitCode: 1, message: `No se pudo ejecutar ${what}: ${result.error.message}` };
  }
  if (result.status !== 0) {
    if (result.signal) {
      return { ok: false, exitCode: 1, message: `${what} terminó por señal ${result.signal}` };
    }
    return {
      ok: false,
      exitCode: result.status ?? 1,
      message: `${what} falló con código ${result.status ?? 1}`,
    };
  }
  return { ok: true };
}

export function rpmCompressionType(conf: {
  bundle?: { linux?: { rpm?: { compression?: { type?: string } | string } } };
}): string | undefined {
  const compression = conf.bundle?.linux?.rpm?.compression;
  if (compression === undefined || compression === null) return undefined;
  if (typeof compression === "string") return compression;
  return compression.type;
}

/**
 * rpm-rs 0.16 + gzip de ~350 MB de CEF tarda decenas de minutos en cualquier
 * distro (Fedora/RHEL/openSUSE y también el lab Debian/Ubuntu). `none` es
 * política de producto, no un atajo de Ubuntu.
 */
export function assertRpmCompressionNone(type: string | undefined): void {
  if (type !== RPM_COMPRESSION_NONE) {
    throw new Error(
      `rpm compression debe ser ${RPM_COMPRESSION_NONE} (staging ~350 MB). Recibido: ${type ?? "undefined"}`,
    );
  }
}

export function appImagePluginUrl(linuxdeployArch: string): string {
  return APPIMAGE_PLUGIN_URL.replace("{arch}", linuxdeployArch);
}

export type AppImagePluginResolution =
  | { kind: "override"; path: string }
  | { kind: "cached"; path: string }
  | { kind: "download"; path: string; url: string };

export function planAppImagePlugin(opts: {
  override: string | undefined;
  toolsDir: string;
  exists: (pluginPath: string) => boolean;
  arch: string;
}): AppImagePluginResolution {
  if (opts.override && opts.override.length > 0) {
    if (!opts.exists(opts.override)) {
      throw new Error(`IDIOTEQUE_APPIMAGE_PLUGIN apunta a ${opts.override}, que no existe`);
    }
    return { kind: "override", path: opts.override };
  }
  const plugin = path.join(opts.toolsDir, "linuxdeploy-plugin-appimage.AppImage");
  if (opts.exists(plugin)) return { kind: "cached", path: plugin };
  return { kind: "download", path: plugin, url: appImagePluginUrl(opts.arch) };
}

export function appImageBundleArgs(plan: Pick<BuildPlan, "debug" | "target">): string[] {
  const bundleArgs = ["bundle", "--bundles", "appimage", "--config", APPIMAGE_OVERRIDE_CONFIG];
  if (plan.debug) bundleArgs.push("--debug");
  if (plan.target) bundleArgs.push("--target", plan.target);
  return bundleArgs;
}

export function appDirCefBase(appDir: string, resourceDirName: string): string {
  return path.join(appDir, "usr", "lib", resourceDirName, "cef", "base");
}

export function appDirCefHost(appDir: string): string {
  return path.join(appDir, "usr", "bin", "cef-host");
}

/** linuxdeploy ya escribió el AppDir (AppRun). Inyectar antes rompe libcef. */
export function assertLinuxdeployAppDir(appDir: string): void {
  if (!fs.existsSync(appDir) || !fs.statSync(appDir).isDirectory()) {
    throw new Error(
      `No existe el AppDir (${appDir}); la inyección de CEF es DESPUÉS de linuxdeploy`,
    );
  }
  const appRun = path.join(appDir, "AppRun");
  if (!fs.existsSync(appRun)) {
    throw new Error(
      `AppDir sin AppRun (${appDir}): linuxdeploy no ha terminado; no se inyecta CEF`,
    );
  }
}

export function assertLinuxdeployOutputs(appDir: string, appImage: string): void {
  assertLinuxdeployAppDir(appDir);
  if (!fs.existsSync(appImage)) {
    throw new Error(
      `linuxdeploy no produjo ${appImage}; no se inyecta CEF ni se reempaqueta`,
    );
  }
}

export interface InjectPaths {
  cefBaseDir: string;
  binariesDir: string;
}

export function defaultInjectPaths(): InjectPaths {
  return { cefBaseDir: CEF_BASE_DIR, binariesDir: BINARIES_DIR };
}

/** Ruta de manifest que no puede salir de `usr/lib/<app>/cef/base`. */
export function resolveManifestDest(baseDest: string, relative: string): string {
  if (!relative || relative.length === 0) {
    throw new Error("Entrada de manifest sin path");
  }
  if (path.isAbsolute(relative)) {
    throw new Error(`Ruta de manifest fuera del AppDir: ${relative}`);
  }
  const parts = relative.split(/[\\/]/).filter((part) => part.length > 0 && part !== ".");
  if (parts.includes("..") || parts.includes("")) {
    throw new Error(`Ruta de manifest fuera del AppDir: ${relative}`);
  }
  const full = path.join(baseDest, ...parts);
  const rel = path.relative(baseDest, full);
  if (rel.startsWith("..") || path.isAbsolute(rel)) {
    throw new Error(`Ruta de manifest fuera del AppDir: ${relative}`);
  }
  return full;
}

// ---------------------------------------------------------------------------
// Ejecución
// ---------------------------------------------------------------------------

function fail(message: string): never {
  console.error(`[tauri] ${message}`);
  process.exit(1);
}

function exitWith(result: SpawnSyncReturns<Buffer>, what: string): void {
  const interpreted = interpretChildExit(result, what);
  if (!interpreted.ok) {
    console.error(`[tauri] ${interpreted.message}`);
    process.exit(interpreted.exitCode);
  }
}

function runTauri(args: string[], env: NodeJS.ProcessEnv = process.env): SpawnSyncReturns<Buffer> {
  return spawnSync(TAURI_BIN, args, { cwd: ROOT, stdio: "inherit", env });
}

function runCefPrepare(): void {
  if (nextDevStep([], undefined) !== "cef-prepare") {
    fail("dev debe correr cef:prepare antes del CLI");
  }
  console.log("[tauri] Preparando CEF (cef-host + base) antes de arrancar el CLI…");
  const { command, args } = cefPrepareArgv(process.execPath);
  const result = spawnSync(command, [...args], {
    cwd: ROOT,
    stdio: "inherit",
    env: process.env,
  });
  const interpreted = interpretChildExit(result, "bun run cef:prepare");
  if (nextDevStep(["cef-prepare"], { ok: interpreted.ok }) === "abort") {
    if (!interpreted.ok) {
      console.error(`[tauri] ${interpreted.message}`);
      process.exit(interpreted.exitCode);
    }
    fail("cef:prepare falló; no se lanza tauri dev");
  }
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
export function injectCefIntoAppDir(
  appDir: string,
  resourceDirName: string,
  triple: string,
  paths: InjectPaths = defaultInjectPaths(),
): void {
  assertLinuxdeployAppDir(appDir);

  const manifestPath = path.join(paths.cefBaseDir, "manifest.json");
  if (!fs.existsSync(manifestPath)) {
    throw new Error(`No existe ${manifestPath}; ejecuta \`bun run cef:prepare\``);
  }
  const hostSrc = path.join(paths.binariesDir, `cef-host-${triple}`);
  if (!fs.existsSync(hostSrc)) {
    throw new Error(`No existe ${hostSrc}; ejecuta \`bun run cef:prepare\``);
  }

  const baseDest = appDirCefBase(appDir, resourceDirName);
  const hostDest = appDirCefHost(appDir);
  fs.rmSync(path.dirname(baseDest), { recursive: true, force: true });
  fs.rmSync(hostDest, { force: true });

  console.log(`[tauri] Inyectando cef-base → ${path.relative(appDir, baseDest)}`);
  copyTree(paths.cefBaseDir, baseDest);
  console.log(`[tauri] Inyectando cef-host → ${path.relative(appDir, hostDest)}`);
  copyPreservingMode(hostSrc, hostDest);
  fs.chmodSync(hostDest, 0o755);

  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as { files?: ManifestFile[] };
  for (const file of manifest.files ?? []) {
    const full = resolveManifestDest(baseDest, file.path);
    if (!fs.existsSync(full)) {
      throw new Error(`Falta ${file.path} en el AppDir tras la inyección`);
    }
    const size = fs.statSync(full).size;
    if (size !== file.size) {
      throw new Error(
        `Tamaño de ${file.path} en el AppDir (${size}) no coincide con el manifest (${file.size})`,
      );
    }
  }
  const sandbox = path.join(baseDest, "chrome-sandbox");
  if (fs.existsSync(sandbox) && (fs.statSync(sandbox).mode & 0o111) === 0) {
    throw new Error("chrome-sandbox perdió el bit de ejecución en el AppDir");
  }
}

function resolveAppImagePlugin(arch: string): string {
  let planned: AppImagePluginResolution;
  try {
    planned = planAppImagePlugin({
      override: process.env.IDIOTEQUE_APPIMAGE_PLUGIN,
      toolsDir: tauriToolsDir(),
      exists: (pluginPath) => fs.existsSync(pluginPath),
      arch,
    });
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
  }
  if (planned.kind !== "download") return planned.path;

  console.log(`[tauri] Descargando linuxdeploy-plugin-appimage desde ${planned.url}`);
  fs.mkdirSync(path.dirname(planned.path), { recursive: true });
  const download = spawnSync("curl", ["-fL", "--retry", "3", "-o", planned.path, planned.url], {
    stdio: "inherit",
    env: process.env,
  });
  if (download.error || download.status !== 0) {
    fs.rmSync(planned.path, { force: true });
    fail("No se pudo descargar linuxdeploy-plugin-appimage (o instala curl, o fija IDIOTEQUE_APPIMAGE_PLUGIN)");
  }
  fs.chmodSync(planned.path, 0o770);
  return planned.path;
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

  const completed: AppImageStep[] = [];
  if (nextAppImageStep(completed) !== "linuxdeploy") {
    fail("la AppImage debe empezar por linuxdeploy sin CEF");
  }
  console.log("[tauri] Fase AppImage 1/3: tauri bundle sin CEF (linuxdeploy no debe tocar libcef)");
  exitWith(runTauri(appImageBundleArgs(plan), env), "tauri bundle --bundles appimage");
  completed.push("linuxdeploy");

  try {
    assertLinuxdeployOutputs(appDir, appImage);
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
  }

  if (!canInjectCefAfterLinuxdeploy(completed)) {
    fail("inyectar CEF solo DESPUÉS de linuxdeploy");
  }
  console.log("[tauri] Fase AppImage 2/3: inyectar el runtime CEF en el AppDir");
  try {
    injectCefIntoAppDir(appDir, names.resourceDirName, triple);
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
  }
  completed.push("inject-cef");

  if (nextAppImageStep(completed) !== "repack") {
    fail("el squashfs va después de inyectar CEF");
  }

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
  completed.push("repack");
  if (nextAppImageStep(completed) !== "done") fail("faltan fases de la AppImage");
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
    if (nextDevStep(["cef-prepare"], { ok: true }) !== "tauri-dev") {
      fail("cef:prepare falló; no se lanza tauri dev");
    }
    exitWith(runTauri(args), "tauri dev");
    return;
  }

  if (subcommand === "build") {
    const plan = planBuild(args);
    const tmpDir = buildTmpDir();
    if (isSystemTmpDir(tmpDir)) {
      fail("TMPDIR de build no puede ser /tmp, /var/tmp ni /dev/shm (usrquota / tmpfs)");
    }
    fs.mkdirSync(tmpDir, { recursive: true });
    const surfaces = linuxSurfacesInPlay(args);
    const env = buildEnv(process.env, tmpDir);
    for (const surface of surfaces) {
      const surfaceEnv = envForLinuxSurface(surface, process.env, tmpDir);
      if (surfaceEnv.TMPDIR !== env.TMPDIR) {
        fail(`TMPDIR de ${surface} (${surfaceEnv.TMPDIR}) distinto del staging (${env.TMPDIR})`);
      }
    }
    if (surfaces.includes("rpm")) {
      try {
        const confPath = path.join(SRC_TAURI, "tauri.conf.json");
        const conf = JSON.parse(fs.readFileSync(confPath, "utf8")) as Parameters<
          typeof rpmCompressionType
        >[0];
        assertRpmCompressionNone(rpmCompressionType(conf));
      } catch (error) {
        fail(error instanceof Error ? error.message : String(error));
      }
    }
    exitWith(runTauri(plan.buildArgs, env), "tauri build");
    if (plan.appimage) buildAppImage(plan, env);
    return;
  }

  exitWith(runTauri(args), `tauri ${subcommand ?? ""}`.trim());
}

const invokedDirectly =
  typeof process.argv[1] === "string" && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) main(process.argv);
