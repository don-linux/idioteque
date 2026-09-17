// svelte-check has no @types/node; the test runner provides node:fs at runtime.
// @ts-expect-error Node built-in used only in this guard test.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

/**
 * Librerías del sistema que `libcef.so` (Chromium) necesita y que no trae el
 * deb/rpm por defecto de Tauri (webkit2gtk y gtk3). Sacadas de
 * `ldd src-tauri/cef-base/libcef.so`; el deb usa los nombres clásicos de
 * paquete (Ubuntu 24.04+ hace `Provides` de ellos desde los `t64`) y el rpm
 * exige el soname, que resuelven dnf/zypper en cualquier distro.
 */
const CHROMIUM_RUNTIME_LIBS: ReadonlyArray<{ soname: string; deb: string }> = [
  { soname: "libnss3.so", deb: "libnss3" },
  { soname: "libnspr4.so", deb: "libnspr4" },
  { soname: "libasound.so.2", deb: "libasound2" },
  { soname: "libatk-1.0.so.0", deb: "libatk1.0-0" },
  { soname: "libatk-bridge-2.0.so.0", deb: "libatk-bridge2.0-0" },
  { soname: "libatspi.so.0", deb: "libatspi2.0-0" },
  { soname: "libcairo.so.2", deb: "libcairo2" },
  { soname: "libcups.so.2", deb: "libcups2" },
  { soname: "libdbus-1.so.3", deb: "libdbus-1-3" },
  { soname: "libdrm.so.2", deb: "libdrm2" },
  { soname: "libexpat.so.1", deb: "libexpat1" },
  { soname: "libgbm.so.1", deb: "libgbm1" },
  { soname: "libglib-2.0.so.0", deb: "libglib2.0-0" },
  { soname: "libpango-1.0.so.0", deb: "libpango-1.0-0" },
  { soname: "libX11.so.6", deb: "libx11-6" },
  { soname: "libxcb.so.1", deb: "libxcb1" },
  { soname: "libXcomposite.so.1", deb: "libxcomposite1" },
  { soname: "libXdamage.so.1", deb: "libxdamage1" },
  { soname: "libXext.so.6", deb: "libxext6" },
  { soname: "libXfixes.so.3", deb: "libxfixes3" },
  { soname: "libXi.so.6", deb: "libxi6" },
  { soname: "libxkbcommon.so.0", deb: "libxkbcommon0" },
  { soname: "libXrandr.so.2", deb: "libxrandr2" },
  { soname: "libXrender.so.1", deb: "libxrender1" },
  { soname: "libXRes.so.1", deb: "libxres1" },
  { soname: "libatomic.so.1", deb: "libatomic1" },
];

interface TauriConf {
  build: { beforeDevCommand: string; beforeBuildCommand: string };
  bundle: {
    targets: string[] | string;
    linux: {
      deb: { depends: string[] };
      rpm: { depends: string[]; compression: { type: string } };
    };
  };
}

function readConf(): TauriConf {
  return JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")) as TauriConf;
}

describe("tauri.conf.json: dependencias de Chromium en deb y rpm", () => {
  it("el deb declara cada librería con su nombre clásico de paquete", () => {
    const depends = readConf().bundle.linux.deb.depends;
    for (const lib of CHROMIUM_RUNTIME_LIBS) {
      expect(depends, lib.soname).toContain(lib.deb);
    }
    expect(new Set(depends).size).toBe(depends.length);
  });

  it("el rpm exige cada librería por soname de 64 bits", () => {
    const depends = readConf().bundle.linux.rpm.depends;
    for (const lib of CHROMIUM_RUNTIME_LIBS) {
      expect(depends, lib.soname).toContain(`${lib.soname}()(64bit)`);
    }
    for (const entry of depends) {
      expect(entry).toMatch(/^lib.+\.so(\.\d+)*\(\)\(64bit\)$/);
    }
    expect(new Set(depends).size).toBe(depends.length);
  });

  it("deb y rpm cubren exactamente la misma lista", () => {
    const { deb, rpm } = readConf().bundle.linux;
    expect(deb.depends.length).toBe(CHROMIUM_RUNTIME_LIBS.length);
    expect(rpm.depends.length).toBe(CHROMIUM_RUNTIME_LIBS.length);
  });
});

describe("tauri.conf.json: pipeline de build", () => {
  it("el rpm va sin compresión (rpm-rs tarda decenas de minutos con 350 MB de CEF)", () => {
    expect(readConf().bundle.linux.rpm.compression).toEqual({ type: "none" });
  });

  it("la AppImage no es target de Tauri: la construye scripts/tauri.ts tras inyectar CEF", () => {
    const targets = readConf().bundle.targets;
    expect(Array.isArray(targets)).toBe(true);
    expect(targets).not.toContain("appimage");
    expect(targets).toContain("deb");
  });

  it("cef:prepare no va en beforeDevCommand (el CLI abandona a los 180 s) pero sí en beforeBuildCommand", () => {
    const { beforeDevCommand, beforeBuildCommand } = readConf().build;
    expect(beforeDevCommand).not.toContain("cef:prepare");
    expect(beforeBuildCommand).toContain("cef:prepare");
  });
});
