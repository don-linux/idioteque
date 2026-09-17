import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import {
  APPIMAGE_OVERRIDE_CONFIG,
  APPIMAGE_STEPS,
  LINUX_PACKAGE_SURFACES,
  RPM_COMPRESSION_NONE,
  appDirCefBase,
  appDirCefHost,
  appImageArch,
  appImageBundleArgs,
  appImagePluginUrl,
  archOfTriple,
  artifactNames,
  assertLinuxdeployAppDir,
  assertLinuxdeployOutputs,
  assertRpmCompressionNone,
  buildEnv,
  buildTmpDir,
  canInjectCefAfterLinuxdeploy,
  cefPrepareArgv,
  envForLinuxSurface,
  extractBundles,
  extractTarget,
  hasFlag,
  injectCefIntoAppDir,
  interpretChildExit,
  isSystemTmpDir,
  linuxSurfacesInPlay,
  nextAppImageStep,
  nextDevStep,
  planAppImagePlugin,
  planBuild,
  profileDir,
  resolveManifestDest,
  rpmCompressionType,
  ROOT,
  subcommandOf,
  tauriToolsDir,
  toolsArch,
} from "./tauri";

describe("subcommandOf", () => {
  it("devuelve el primer argumento que no es flag", () => {
    expect(subcommandOf(["dev"])).toBe("dev");
    expect(subcommandOf(["-v", "build", "--debug"])).toBe("build");
    expect(subcommandOf(["--verbose", "-vv", "icon", "x.png"])).toBe("icon");
  });

  it("es undefined sin subcomando", () => {
    expect(subcommandOf([])).toBeUndefined();
    expect(subcommandOf(["--help"])).toBeUndefined();
  });
});

describe("extractBundles", () => {
  it("null cuando no se pasa --bundles", () => {
    expect(extractBundles(["build", "--debug"])).toBeNull();
  });

  it("acepta varios valores separados por espacio y por coma", () => {
    expect(extractBundles(["build", "--bundles", "deb", "rpm"])).toEqual({
      bundles: ["deb", "rpm"],
      span: [1, 2, 3],
    });
    expect(extractBundles(["build", "-b", "deb,appimage", "--debug"])).toEqual({
      bundles: ["deb", "appimage"],
      span: [1, 2],
    });
    expect(extractBundles(["build", "--bundles=appimage"])).toEqual({
      bundles: ["appimage"],
      span: [1],
    });
  });

  it("para en el siguiente flag y no mira detrás de --", () => {
    expect(extractBundles(["build", "--bundles", "deb", "--debug", "rpm"])).toEqual({
      bundles: ["deb"],
      span: [1, 2],
    });
    expect(extractBundles(["build", "--", "--bundles", "deb"])).toBeNull();
  });
});

describe("extractTarget / hasFlag", () => {
  it("lee --target en sus dos formas", () => {
    expect(extractTarget(["build", "--target", "aarch64-unknown-linux-gnu"])).toBe("aarch64-unknown-linux-gnu");
    expect(extractTarget(["build", "-t=x86_64-unknown-linux-gnu"])).toBe("x86_64-unknown-linux-gnu");
    expect(extractTarget(["build"])).toBeUndefined();
  });

  it("hasFlag ignora lo que va detrás de --", () => {
    expect(hasFlag(["build", "--debug"], "--debug", "-d")).toBe(true);
    expect(hasFlag(["build", "--", "--debug"], "--debug")).toBe(false);
  });
});

describe("planBuild", () => {
  it("sin --bundles: build tal cual y AppImage aparte en Linux", () => {
    const plan = planBuild(["build"], "linux");
    expect(plan).toEqual({ buildArgs: ["build"], appimage: true, debug: false, target: undefined });
  });

  it("fuera de Linux no hay fase AppImage", () => {
    expect(planBuild(["build"], "darwin").appimage).toBe(false);
    expect(planBuild(["build", "--bundles", "appimage"], "win32").buildArgs).toEqual([
      "build",
      "--bundles",
      "appimage",
    ]);
  });

  it("--bundles sin appimage es passthrough", () => {
    const plan = planBuild(["build", "--bundles", "deb", "rpm"], "linux");
    expect(plan.appimage).toBe(false);
    expect(plan.buildArgs).toEqual(["build", "--bundles", "deb", "rpm"]);
  });

  it("--bundles con appimage la quita de la lista y la hace por la fase especial", () => {
    const plan = planBuild(["build", "--bundles", "deb", "appimage", "--debug"], "linux");
    expect(plan.appimage).toBe(true);
    expect(plan.debug).toBe(true);
    expect(plan.buildArgs).toEqual(["build", "--debug", "--bundles", "deb"]);
  });

  it("solo appimage: el build va con --no-bundle", () => {
    const plan = planBuild(["build", "-b", "appimage"], "linux");
    expect(plan.appimage).toBe(true);
    expect(plan.buildArgs).toEqual(["build", "--no-bundle"]);
  });

  it("--no-bundle y --help son passthrough", () => {
    expect(planBuild(["build", "--no-bundle"], "linux").appimage).toBe(false);
    expect(planBuild(["build", "--help"], "linux").appimage).toBe(false);
  });

  it("propaga --target", () => {
    const plan = planBuild(["build", "--target", "aarch64-unknown-linux-gnu"], "linux");
    expect(plan.target).toBe("aarch64-unknown-linux-gnu");
    expect(plan.appimage).toBe(true);
  });
});

describe("arquitecturas y nombres de artefactos", () => {
  it("mapea la arquitectura de Rust a la del nombre de la AppImage y a la de linuxdeploy", () => {
    expect(appImageArch("x86_64")).toBe("amd64");
    expect(appImageArch("aarch64")).toBe("aarch64");
    expect(appImageArch("i686")).toBe("i386");
    expect(appImageArch("armv7")).toBe("armhf");
    expect(() => appImageArch("riscv64gc")).toThrow();
    expect(toolsArch("x86_64")).toBe("x86_64");
    expect(toolsArch("armv7")).toBe("armhf");
    expect(archOfTriple("x86_64-unknown-linux-gnu")).toBe("x86_64");
  });

  it("nombra el AppDir y la AppImage como el bundler de Tauri", () => {
    expect(artifactNames({ productName: "idioteque", version: "0.1.0" }, "x86_64")).toEqual({
      appDirName: "idioteque.AppDir",
      appImageName: "idioteque_0.1.0_amd64.AppImage",
      resourceDirName: "idioteque",
    });
  });

  it("profileDir sigue el layout de cargo con y sin --target", () => {
    const base = path.join(ROOT, "src-tauri", "target");
    expect(profileDir(undefined, false)).toBe(path.join(base, "release"));
    expect(profileDir(undefined, true)).toBe(path.join(base, "debug"));
    expect(profileDir("aarch64-unknown-linux-gnu", false)).toBe(
      path.join(base, "aarch64-unknown-linux-gnu", "release"),
    );
  });
});

describe("config de override y directorio de herramientas", () => {
  it("la AppImage se bundlea sin resources ni externalBin", () => {
    expect(JSON.parse(APPIMAGE_OVERRIDE_CONFIG)).toEqual({ bundle: { resources: [], externalBin: [] } });
  });

  it("usa XDG_CACHE_HOME si está, si no ~/.cache", () => {
    expect(tauriToolsDir({ XDG_CACHE_HOME: "/x/cache" }, "/home/u")).toBe("/x/cache/tauri");
    expect(tauriToolsDir({}, "/home/u")).toBe("/home/u/.cache/tauri");
    expect(tauriToolsDir({ XDG_CACHE_HOME: "" }, "/home/u")).toBe("/home/u/.cache/tauri");
  });
});

describe("temporales del build fuera de /tmp", () => {
  it("el TMPDIR del build vive dentro de src-tauri/target", () => {
    expect(buildTmpDir()).toBe(path.join(ROOT, "src-tauri", "target", "tmp"));
  });

  it("pone TMPDIR si el usuario no lo trae", () => {
    const env = buildEnv({ PATH: "/bin" }, "/repo/src-tauri/target/tmp");
    expect(env).toEqual({ PATH: "/bin", TMPDIR: "/repo/src-tauri/target/tmp" });
    expect(buildEnv({ TMPDIR: "" }, "/x").TMPDIR).toBe("/x");
  });

  it("respeta un TMPDIR del usuario y no muta el entorno original", () => {
    const original = { PATH: "/bin", TMPDIR: "/mnt/scratch" };
    const env = buildEnv(original, "/x");
    expect(env.TMPDIR).toBe("/mnt/scratch");
    expect(env).not.toBe(original);
    expect(original).toEqual({ PATH: "/bin", TMPDIR: "/mnt/scratch" });
  });
});

describe("superficies Linux: deb + rpm + AppImage (no solo Ubuntu)", () => {
  it("el producto empaqueta las tres superficies, no un único deb de Ubuntu", () => {
    expect(LINUX_PACKAGE_SURFACES).toEqual(["deb", "rpm", "appimage"]);
    expect(linuxSurfacesInPlay(["build"], "linux")).toEqual(["deb", "rpm", "appimage"]);
  });

  it("rpm-only es Fedora/RHEL/openSUSE; deb-only es familia Debian; AppImage es host cualquiera", () => {
    expect(linuxSurfacesInPlay(["build", "--bundles", "rpm"], "linux")).toEqual(["rpm"]);
    expect(linuxSurfacesInPlay(["build", "--bundles", "deb"], "linux")).toEqual(["deb"]);
    expect(linuxSurfacesInPlay(["build", "-b", "appimage"], "linux")).toEqual(["appimage"]);
    expect(linuxSurfacesInPlay(["build", "--bundles", "deb", "rpm", "appimage"], "linux")).toEqual([
      "deb",
      "rpm",
      "appimage",
    ]);
  });

  it("fuera de Linux o con --help/--no-bundle no finge un paquete Ubuntu", () => {
    expect(linuxSurfacesInPlay(["build"], "darwin")).toEqual([]);
    expect(linuxSurfacesInPlay(["build"], "win32")).toEqual([]);
    expect(linuxSurfacesInPlay(["build", "--help"], "linux")).toEqual([]);
    expect(linuxSurfacesInPlay(["build", "--no-bundle"], "linux")).toEqual([]);
  });

  it("TMPDIR=src-tauri/target/tmp para deb Y rpm Y AppImage", () => {
    const tmp = buildTmpDir();
    expect(tmp).toBe(path.join(ROOT, "src-tauri", "target", "tmp"));
    expect(isSystemTmpDir(tmp)).toBe(false);
    const base = { PATH: "/usr/bin" };
    const tmpdirs = LINUX_PACKAGE_SURFACES.map(
      (surface) => envForLinuxSurface(surface, base, tmp).TMPDIR,
    );
    expect(new Set(tmpdirs)).toEqual(new Set([tmp]));
    for (const surface of ["deb", "rpm", "appimage"] as const) {
      expect(envForLinuxSurface(surface, base, tmp).TMPDIR).not.toBe("/tmp");
      expect(envForLinuxSurface(surface, base, tmp).TMPDIR).not.toBe("/var/tmp");
    }
  });

  it("no trata /tmp /var/tmp /dev/shm ni XDG_RUNTIME_DIR como staging por defecto", () => {
    expect(isSystemTmpDir("/tmp")).toBe(true);
    expect(isSystemTmpDir("/var/tmp")).toBe(true);
    expect(isSystemTmpDir("/dev/shm")).toBe(true);
    expect(isSystemTmpDir("/run/user/1000")).toBe(true);
    expect(isSystemTmpDir("/run/user/1000/tmp")).toBe(true);
    expect(isSystemTmpDir(path.join(ROOT, "src-tauri", "target", "tmp"))).toBe(false);
  });

  it("usrquota de systemd ≥258 no es un atajo de Ubuntu: el default sigue fuera de /tmp", () => {
    const fedoraTmpfs = "/tmp";
    const debianTmpfs = "/tmp";
    expect(isSystemTmpDir(fedoraTmpfs)).toBe(true);
    expect(isSystemTmpDir(debianTmpfs)).toBe(true);
    expect(buildTmpDir()).not.toBe(fedoraTmpfs);
    expect(envForLinuxSurface("rpm", {}, buildTmpDir()).TMPDIR).toBe(buildTmpDir());
    expect(envForLinuxSurface("deb", {}, buildTmpDir()).TMPDIR).toBe(buildTmpDir());
    expect(envForLinuxSurface("appimage", {}, buildTmpDir()).TMPDIR).toBe(buildTmpDir());
  });

  it("un TMPDIR de usuario se respeta en las tres superficies y no se muta", () => {
    const original = { TMPDIR: "/mnt/scratch", PATH: "/bin" };
    for (const surface of LINUX_PACKAGE_SURFACES) {
      const env = envForLinuxSurface(surface, original, buildTmpDir());
      expect(env.TMPDIR).toBe("/mnt/scratch");
      expect(env).not.toBe(original);
    }
    expect(original.TMPDIR).toBe("/mnt/scratch");
  });
});

describe("rpm compression none", () => {
  it("acepta none y rechaza gzip/xz/zstd/ausente (Fedora y Debian por igual)", () => {
    assertRpmCompressionNone("none");
    assertRpmCompressionNone(RPM_COMPRESSION_NONE);
    expect(() => assertRpmCompressionNone("gzip")).toThrow(/none/);
    expect(() => assertRpmCompressionNone("xz")).toThrow(/none/);
    expect(() => assertRpmCompressionNone("zstd")).toThrow(/none/);
    expect(() => assertRpmCompressionNone(undefined)).toThrow(/undefined/);
  });

  it("lee compression como objeto Tauri o como string suelto", () => {
    expect(rpmCompressionType({ bundle: { linux: { rpm: { compression: { type: "none" } } } } })).toBe(
      "none",
    );
    expect(rpmCompressionType({ bundle: { linux: { rpm: { compression: "gzip" } } } })).toBe("gzip");
    expect(rpmCompressionType({})).toBeUndefined();
  });
});

describe("cef:prepare debe fallar el wrapper, no seguir al CLI", () => {
  it("el argv es bun run cef:prepare (no un apt/dnf del lab)", () => {
    expect(cefPrepareArgv("/opt/bun/bin/bun")).toEqual({
      command: "/opt/bun/bin/bun",
      args: ["run", "cef:prepare"],
    });
  });

  it("status distinto de 0, señal o ENOENT abortan; 0 deja paso a tauri dev", () => {
    expect(interpretChildExit({ status: 1, signal: null }, "bun run cef:prepare")).toEqual({
      ok: false,
      exitCode: 1,
      message: "bun run cef:prepare falló con código 1",
    });
    expect(interpretChildExit({ status: 12, signal: null }, "bun run cef:prepare").ok).toBe(false);
    expect(interpretChildExit({ status: 1, signal: "SIGKILL" }, "bun run cef:prepare")).toMatchObject({
      ok: false,
      message: "bun run cef:prepare terminó por señal SIGKILL",
    });
    expect(
      interpretChildExit({ error: new Error("spawn ENOENT"), status: null, signal: null }, "bun run cef:prepare")
        .ok,
    ).toBe(false);
    expect(interpretChildExit({ status: 0, signal: null }, "bun run cef:prepare")).toEqual({ ok: true });
  });

  it("no se puede saltar prepare ni lanzar tauri dev si prepare falló", () => {
    expect(nextDevStep([], undefined)).toBe("cef-prepare");
    expect(nextDevStep(["cef-prepare"], { ok: false })).toBe("abort");
    expect(nextDevStep(["cef-prepare"], undefined)).toBe("abort");
    expect(nextDevStep(["cef-prepare"], { ok: true })).toBe("tauri-dev");
    expect(nextDevStep(["cef-prepare", "tauri-dev"], { ok: true })).toBe("done");
    expect(nextDevStep(["tauri-dev"], { ok: true })).toBe("cef-prepare");
  });
});

describe("AppImage: inyectar CEF DESPUÉS de linuxdeploy", () => {
  it("el orden es linuxdeploy → inject-cef → repack y no se puede colar la inyección", () => {
    expect(APPIMAGE_STEPS).toEqual(["linuxdeploy", "inject-cef", "repack"]);
    expect(nextAppImageStep([])).toBe("linuxdeploy");
    expect(canInjectCefAfterLinuxdeploy([])).toBe(false);
    expect(canInjectCefAfterLinuxdeploy(["linuxdeploy"])).toBe(true);
    expect(canInjectCefAfterLinuxdeploy(["inject-cef"])).toBe(false);
    expect(canInjectCefAfterLinuxdeploy(["linuxdeploy", "inject-cef"])).toBe(false);
    expect(canInjectCefAfterLinuxdeploy(["linuxdeploy", "inject-cef", "repack"])).toBe(false);
    expect(nextAppImageStep(["linuxdeploy"])).toBe("inject-cef");
    expect(nextAppImageStep(["linuxdeploy", "inject-cef"])).toBe("repack");
    expect(nextAppImageStep(["linuxdeploy", "inject-cef", "repack"])).toBe("done");
  });

  it("linuxdeploy corre sin resources/externalBin para no patchelf-ear libcef", () => {
    expect(JSON.parse(APPIMAGE_OVERRIDE_CONFIG)).toEqual({ bundle: { resources: [], externalBin: [] } });
    const args = appImageBundleArgs({ debug: true, target: "aarch64-unknown-linux-gnu" });
    expect(args).toEqual([
      "bundle",
      "--bundles",
      "appimage",
      "--config",
      APPIMAGE_OVERRIDE_CONFIG,
      "--debug",
      "--target",
      "aarch64-unknown-linux-gnu",
    ]);
    expect(args.join(" ")).not.toMatch(/cef-base|libcef|externalBin/);
  });

  it("el plugin usa el arch de linuxdeploy (x86_64), no el amd64 de Debian/AppImage", () => {
    expect(appImagePluginUrl(toolsArch("x86_64"))).toContain("linuxdeploy-plugin-appimage-x86_64.AppImage");
    expect(appImagePluginUrl(toolsArch("x86_64"))).not.toContain("amd64");
    expect(appImagePluginUrl(toolsArch("aarch64"))).toContain("aarch64");
    expect(appImagePluginUrl(toolsArch("armv7"))).toContain("armhf");
    expect(appImageArch("x86_64")).toBe("amd64");
  });

  it("plugin: override inexistente falla; caché gana; si no, descarga", () => {
    const exists = (pluginPath: string) => pluginPath === "/cache/tauri/linuxdeploy-plugin-appimage.AppImage";
    expect(() =>
      planAppImagePlugin({ override: "/no/plugin", toolsDir: "/cache/tauri", exists, arch: "x86_64" }),
    ).toThrow(/IDIOTEQUE_APPIMAGE_PLUGIN/);
    expect(
      planAppImagePlugin({ override: undefined, toolsDir: "/cache/tauri", exists, arch: "x86_64" }),
    ).toEqual({ kind: "cached", path: "/cache/tauri/linuxdeploy-plugin-appimage.AppImage" });
    expect(
      planAppImagePlugin({
        override: "",
        toolsDir: "/tmp/tools",
        exists: () => false,
        arch: "aarch64",
      }),
    ).toEqual({
      kind: "download",
      path: "/tmp/tools/linuxdeploy-plugin-appimage.AppImage",
      url: appImagePluginUrl("aarch64"),
    });
  });
});

const scratchDirs: string[] = [];

afterEach(() => {
  for (const dir of scratchDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

function scratch(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "idq-tauri-wrap-"));
  scratchDirs.push(dir);
  return dir;
}

function writeFile(filePath: string, contents: string | Buffer, mode = 0o644): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, contents);
  fs.chmodSync(filePath, mode);
}

function fakeLinuxdeployAppDir(root: string, product = "idioteque"): string {
  const appDir = path.join(root, `${product}.AppDir`);
  writeFile(path.join(appDir, "AppRun"), "#!/bin/sh\n", 0o755);
  writeFile(path.join(appDir, "usr", "bin", product), "elf", 0o755);
  return appDir;
}

function fakeCefLayout(root: string, triple: string, opts?: { sandboxMode?: number; libcef?: string }) {
  const cefBaseDir = path.join(root, "cef-base");
  const binariesDir = path.join(root, "binaries");
  const libcef = opts?.libcef ?? "LIBCEF";
  const sandbox = "SANDBOX";
  writeFile(path.join(cefBaseDir, "libcef.so"), libcef);
  writeFile(path.join(cefBaseDir, "chrome-sandbox"), sandbox, opts?.sandboxMode ?? 0o755);
  writeFile(
    path.join(cefBaseDir, "manifest.json"),
    JSON.stringify({
      files: [
        { path: "libcef.so", size: Buffer.byteLength(libcef) },
        { path: "chrome-sandbox", size: Buffer.byteLength(sandbox) },
      ],
    }),
  );
  writeFile(path.join(binariesDir, `cef-host-${triple}`), "HOST", 0o755);
  return { cefBaseDir, binariesDir };
}

describe("injectCefIntoAppDir (después de linuxdeploy)", () => {
  const triple = "x86_64-unknown-linux-gnu";

  it("falla si linuxdeploy aún no dejó AppDir/AppRun (inyectar antes)", () => {
    const root = scratch();
    const missing = path.join(root, "idioteque.AppDir");
    const paths = fakeCefLayout(root, triple);
    expect(() => injectCefIntoAppDir(missing, "idioteque", triple, paths)).toThrow(/DESPUÉS de linuxdeploy/);
    expect(() => assertLinuxdeployAppDir(missing)).toThrow(/DESPUÉS de linuxdeploy/);

    fs.mkdirSync(missing);
    expect(() => injectCefIntoAppDir(missing, "idioteque", triple, paths)).toThrow(/AppRun/);
  });

  it("falla si linuxdeploy no produjo el .AppImage", () => {
    const root = scratch();
    const appDir = fakeLinuxdeployAppDir(root);
    expect(() => assertLinuxdeployOutputs(appDir, path.join(root, "missing.AppImage"))).toThrow(
      /no produjo/,
    );
  });

  it("inyecta en usr/lib/<app>/cef/base y usr/bin/cef-host (ruta FHS, no /opt de Ubuntu)", () => {
    const root = scratch();
    const appDir = fakeLinuxdeployAppDir(root);
    writeFile(path.join(root, "idioteque.AppImage"), "squash");
    const paths = fakeCefLayout(root, triple);
    injectCefIntoAppDir(appDir, "idioteque", triple, paths);
    const base = appDirCefBase(appDir, "idioteque");
    expect(base).toBe(path.join(appDir, "usr", "lib", "idioteque", "cef", "base"));
    expect(appDirCefHost(appDir)).toBe(path.join(appDir, "usr", "bin", "cef-host"));
    expect(fs.readFileSync(path.join(base, "libcef.so"), "utf8")).toBe("LIBCEF");
    expect(fs.readFileSync(appDirCefHost(appDir), "utf8")).toBe("HOST");
    expect(fs.statSync(appDirCefHost(appDir)).mode & 0o111).not.toBe(0);
    expect(fs.statSync(path.join(base, "chrome-sandbox")).mode & 0o111).not.toBe(0);
  });

  it("rechaza prepare incompleto: sin manifest, sin sidecar, triple equivocado", () => {
    const root = scratch();
    const appDir = fakeLinuxdeployAppDir(root);
    const paths = fakeCefLayout(root, triple);
    expect(() =>
      injectCefIntoAppDir(appDir, "idioteque", triple, { ...paths, cefBaseDir: path.join(root, "empty") }),
    ).toThrow(/cef:prepare/);
    expect(() => injectCefIntoAppDir(appDir, "idioteque", "aarch64-unknown-linux-gnu", paths)).toThrow(
      /cef:prepare/,
    );
  });

  it("rechaza tamaño distinto al manifest (linuxdeploy/patchelf habría cambiado libcef)", () => {
    const root = scratch();
    const appDir = fakeLinuxdeployAppDir(root);
    const paths = fakeCefLayout(root, triple, { libcef: "SMALL" });
    fs.writeFileSync(
      path.join(paths.cefBaseDir, "manifest.json"),
      JSON.stringify({ files: [{ path: "libcef.so", size: 999 }, { path: "chrome-sandbox", size: 7 }] }),
    );
    expect(() => injectCefIntoAppDir(appDir, "idioteque", triple, paths)).toThrow(/no coincide/);
  });

  it("rechaza un archivo listado que no se copió", () => {
    const root = scratch();
    const appDir = fakeLinuxdeployAppDir(root);
    const paths = fakeCefLayout(root, triple);
    fs.writeFileSync(
      path.join(paths.cefBaseDir, "manifest.json"),
      JSON.stringify({
        files: [
          { path: "libcef.so", size: 6 },
          { path: "locales/en-US.pak", size: 1 },
        ],
      }),
    );
    expect(() => injectCefIntoAppDir(appDir, "idioteque", triple, paths)).toThrow(/Falta locales\/en-US\.pak/);
  });

  it("chrome-sandbox sin bit de ejecución falla (SUID de rpm/deb no se asume en AppImage)", () => {
    const root = scratch();
    const appDir = fakeLinuxdeployAppDir(root);
    const paths = fakeCefLayout(root, triple, { sandboxMode: 0o644 });
    expect(() => injectCefIntoAppDir(appDir, "idioteque", triple, paths)).toThrow(/chrome-sandbox/);
  });

  it("rechaza zip-slip en el manifest (absoluto o ..)", () => {
    const baseDest = path.join(scratch(), "base");
    expect(() => resolveManifestDest(baseDest, "/etc/passwd")).toThrow(/fuera del AppDir/);
    expect(() => resolveManifestDest(baseDest, "../evil")).toThrow(/fuera del AppDir/);
    expect(() => resolveManifestDest(baseDest, "locales/../../etc/shadow")).toThrow(/fuera del AppDir/);
    expect(resolveManifestDest(baseDest, "locales/en-US.pak")).toBe(
      path.join(baseDest, "locales", "en-US.pak"),
    );
  });
});

describe("el wrapper no es un script de Ubuntu", () => {
  it("no hardcodea AppArmor, apt ni shm de 64 MiB", () => {
    const src = fs.readFileSync(new URL("./tauri.ts", import.meta.url), "utf8");
    expect(src).not.toMatch(/\/etc\/apparmor\.d/);
    expect(src).not.toMatch(/apt-get|apparmor_restrict_unprivileged_userns/);
    expect(src).not.toMatch(/64\s*MiB/);
  });
});
