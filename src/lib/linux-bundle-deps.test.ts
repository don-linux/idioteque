// svelte-check has no @types/node; the test runner provides node:fs at runtime.
// @ts-expect-error Node built-in used only in this guard test.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { APPIMAGE_OVERRIDE_CONFIG } from "../../scripts/tauri";

/**
 * Librerías del sistema que `libcef.so` (Chromium) necesita y que no trae el
 * deb/rpm por defecto de Tauri (webkit2gtk y gtk3). Sacadas de
 * `ldd src-tauri/cef-base/libcef.so`.
 *
 * - deb: nombres clásicos Debian (`libnss3`). Ubuntu 24.04+ hace `Provides`
 *   desde los `t64`; el paquete declarado NO es el nombre t64 ni un nombre
 *   `apt` de un PPA. Debian estable, Mint, etc. resuelven el clásico.
 * - rpm: soname ELF `libnss3.so()(64bit)`. Así lo resuelven dnf (Fedora/RHEL)
 *   y zypper (openSUSE). No es un nombre de paquete Fedora (`nss`) ni apt.
 * - AppImage: no declara depends. linuxdeploy trae GTK; NSS/NSPR los aporta
 *   el host (cualquier escritorio con navegador), no Ubuntu.
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

/** Nombres que NO deben aparecer: Ubuntu t64, paquetes Fedora, MAC, metapaquetes. */
const NOT_A_LINUX_SURFACE_DEP = [
  "libnss3t64",
  "libnspr4t64",
  "libasound2t64",
  "libglib2.0-0t64",
  "libatk1.0-0t64",
  "libgtk-3-0t64",
  "nss",
  "nspr",
  "alsa-lib",
  "atk",
  "at-spi2-atk",
  "at-spi2-core",
  "cairo",
  "cups-libs",
  "dbus",
  "mesa-libgbm",
  "glib2",
  "pango",
  "libX11",
  "libXcomposite",
  "apparmor",
  "libapparmor1",
  "apparmor-utils",
  "libselinux1",
  "selinux-policy",
  "selinux-policy-targeted",
  "ubuntu-desktop",
  "ubuntu-restricted-addons",
  "linux-image-generic",
] as const;

interface TauriConf {
  build: { beforeDevCommand: string; beforeBuildCommand: string };
  bundle: {
    targets: string[] | string;
    externalBin?: string[];
    resources?: Record<string, string> | string[];
    linux: {
      deb: { depends: string[] };
      rpm: { depends: string[]; compression: { type: string } };
      appimage?: unknown;
    };
  };
}

function readConf(): TauriConf {
  return JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")) as TauriConf;
}

function isDebianClassicName(name: string): boolean {
  return (
    /^lib[a-z0-9][a-z0-9.+-]*$/.test(name) &&
    !/t64/i.test(name) &&
    !/ubuntu|apparmor|selinux/i.test(name)
  );
}

function isRpm64SonameRequire(entry: string): boolean {
  return /^lib.+\.so(\.\d+)*\(\)\(64bit\)$/.test(entry);
}

describe("tabla Chromium: no es Ubuntu-only ni apt-only", () => {
  it("cada fila es soname ELF + nombre clásico Debian, sin t64 ni Fedora", () => {
    for (const lib of CHROMIUM_RUNTIME_LIBS) {
      expect(lib.soname, lib.deb).toMatch(/^lib.+\.so(\.\d+)*$/);
      expect(isDebianClassicName(lib.deb), lib.deb).toBe(true);
      expect(NOT_A_LINUX_SURFACE_DEP, lib.soname).not.toContain(lib.deb);
    }
  });

  it("incluye NSS/NSPR (AppImage los pide al host; deb/rpm los declaran)", () => {
    const debs = CHROMIUM_RUNTIME_LIBS.map((lib) => lib.deb);
    expect(debs).toContain("libnss3");
    expect(debs).toContain("libnspr4");
  });
});

describe("tauri.conf.json: paridad deb y rpm", () => {
  it("el deb declara cada librería con su nombre clásico de paquete", () => {
    const depends = readConf().bundle.linux.deb.depends;
    for (const lib of CHROMIUM_RUNTIME_LIBS) {
      expect(depends, lib.soname).toContain(lib.deb);
    }
    expect(new Set(depends).size).toBe(depends.length);
  });

  it("el rpm exige cada librería por soname de 64 bits (Fedora/RHEL/openSUSE)", () => {
    const depends = readConf().bundle.linux.rpm.depends;
    for (const lib of CHROMIUM_RUNTIME_LIBS) {
      expect(depends, lib.soname).toContain(`${lib.soname}()(64bit)`);
    }
    for (const entry of depends) {
      expect(isRpm64SonameRequire(entry), entry).toBe(true);
    }
    expect(new Set(depends).size).toBe(depends.length);
  });

  it("deb y rpm son la misma lista (bijección soname ↔ clásico), ni una extra", () => {
    const { deb, rpm } = readConf().bundle.linux;
    const expectedDeb = CHROMIUM_RUNTIME_LIBS.map((lib) => lib.deb);
    const expectedRpm = CHROMIUM_RUNTIME_LIBS.map((lib) => `${lib.soname}()(64bit)`);
    expect(new Set(deb.depends)).toEqual(new Set(expectedDeb));
    expect(new Set(rpm.depends)).toEqual(new Set(expectedRpm));
    expect(deb.depends).toHaveLength(CHROMIUM_RUNTIME_LIBS.length);
    expect(rpm.depends).toHaveLength(CHROMIUM_RUNTIME_LIBS.length);
  });

  it("ninguna lista cuela t64, AppArmor, SELinux ni nombres Fedora/Ubuntu", () => {
    const { deb, rpm } = readConf().bundle.linux;
    const all = [...deb.depends, ...rpm.depends];
    for (const name of NOT_A_LINUX_SURFACE_DEP) {
      expect(all, name).not.toContain(name);
    }
    for (const name of deb.depends) {
      expect(isDebianClassicName(name), name).toBe(true);
    }
    for (const entry of rpm.depends) {
      expect(entry, entry).not.toMatch(/t64|apparmor|selinux|ubuntu/i);
      expect(entry.endsWith("()(32bit)")).toBe(false);
    }
  });
});

describe("tauri.conf.json: pipeline de build", () => {
  it("el rpm va sin compresión (rpm-rs tarda decenas de minutos con 350 MB de CEF)", () => {
    expect(readConf().bundle.linux.rpm.compression).toEqual({ type: "none" });
  });

  it("los targets por defecto son deb y rpm: no appimage (linuxdeploy no ve CEF)", () => {
    const targets = readConf().bundle.targets;
    expect(targets).toEqual(["deb", "rpm"]);
    expect(targets).not.toContain("appimage");
    expect(targets).not.toBe("all");
  });

  it("deb y rpm sí meten CEF (resources + sidecar); AppImage no tiene depends propios", () => {
    const conf = readConf();
    expect(conf.bundle.resources).toEqual({ "cef-base/": "cef/base/" });
    expect(conf.bundle.externalBin).toEqual(["binaries/cef-host"]);
    expect(conf.bundle.linux).not.toHaveProperty("appimage");
  });

  it("cef:prepare no va en beforeDevCommand (el CLI abandona a los 180 s) pero sí en beforeBuildCommand", () => {
    const { beforeDevCommand, beforeBuildCommand } = readConf().build;
    expect(beforeDevCommand).not.toContain("cef:prepare");
    expect(beforeBuildCommand).toContain("cef:prepare");
  });
});

describe("AppImage: CEF no entra en linuxdeploy", () => {
  it("el override del wrapper vacía resources y externalBin", () => {
    expect(JSON.parse(APPIMAGE_OVERRIDE_CONFIG)).toEqual({
      bundle: { resources: [], externalBin: [] },
    });
  });

  it("el wrapper bundlea AppImage sin CEF y lo inyecta después de linuxdeploy", () => {
    const wrap = readFileSync("scripts/tauri.ts", "utf8");
    expect(wrap).toContain("tauri bundle sin CEF (linuxdeploy no debe tocar libcef)");
    expect(wrap).toContain("injectCefIntoAppDir");
    expect(wrap).toContain("linuxdeploy-plugin-appimage");
    expect(wrap).toMatch(/bundleArgs = \["bundle", "--bundles", "appimage", "--config", APPIMAGE_OVERRIDE_CONFIG\]/);
  });
});

describe("build.rs: runtime en el paquete y pin base.json", () => {
  const buildRs = readFileSync("src-tauri/build.rs", "utf8");

  it("en release falta de cef-base/manifest o sidecar es error; en debug solo aviso", () => {
    expect(buildRs).toContain("cef-base");
    expect(buildRs).toContain("manifest.json");
    expect(buildRs).toContain("cef-host-");
    expect(buildRs).toContain("profile == \"release\"");
    expect(buildRs).toContain("cargo:warning=");
    expect(buildRs).toContain("bun run cef:prepare");
  });

  it("el pin de base.json contra Cargo.lock falla el build si no coinciden", () => {
    expect(buildRs).toContain("cef/base.json");
    expect(buildRs).toContain("Cargo.lock");
    expect(buildRs).toMatch(/name = \\"cef\\"/);
    expect(buildRs).toContain("cefVersion");
  });

  it("no reubica el motor a /dev/shm (eso es backing store de Chromium, no libcef)", () => {
    expect(buildRs).not.toMatch(/\/dev\/shm/);
    expect(buildRs).not.toContain("disable-dev-shm-usage");
  });
});
