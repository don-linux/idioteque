import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";

import {
  PrepareError,
  ROOT,
  assertArchiveMatches,
  baseAlreadyPrepared,
  findSdkSlot,
  findVersionDir,
  majorMinorPatch,
  parsePrepareFlags,
  platformOfTriple,
  prepareBase,
  runPrepare,
  sha1Eq,
} from "./cef-prepare";

const LINUX64_NAME = "cef_binary_152.0.6+g708dc14+chromium-152.0.7977.83_linux64_minimal.tar.bz2";
const LINUX64_SHA1 = "9711b86c105fb590da576fe5a829802f1a79d520";

const tempDirs: string[] = [];

function tempDir(prefix: string): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `cef-prepare-${prefix}-`));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop();
    if (dir) fs.rmSync(dir, { recursive: true, force: true });
  }
});

function writeJson(filePath: string, value: unknown): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function fakeBaseJson(overrides: Record<string, unknown> = {}) {
  return {
    cefVersion: "152.0.6+g708dc14+chromium-152.0.7977.83",
    chromiumVersion: "152.0.7977.83",
    hostApiVersion: 15200,
    apiVersionMin: 13300,
    files: {
      linux64: { name: LINUX64_NAME, sha1: LINUX64_SHA1, size: 321503907 },
    },
    ...overrides,
  };
}

function platformFile(overrides: Partial<{ name: string; sha1: string; size: number }> = {}) {
  return { name: LINUX64_NAME, sha1: LINUX64_SHA1, size: 321503907, ...overrides };
}

function fixturePaths(root: string) {
  return {
    srcTauri: root,
    baseJsonPath: path.join(root, "base.json"),
    sdkRoot: path.join(root, "sdk"),
    cefBaseDir: path.join(root, "cef-base"),
    binariesDir: path.join(root, "binaries"),
  };
}

function throwIfPrepareCalled(which: string): never {
  throw new Error(`no debía llamarse ${which} (ni cargo ni SDK)`);
}

describe("parsePrepareFlags / --skip-*", () => {
  it("sin flags no omite nada", () => {
    expect(parsePrepareFlags([])).toEqual({ skipHost: false, skipBase: false, force: false, unknown: [] });
  });

  it("reconoce --skip-host, --skip-base y --force", () => {
    expect(parsePrepareFlags(["--skip-host"])).toMatchObject({ skipHost: true, skipBase: false, force: false });
    expect(parsePrepareFlags(["--skip-base"])).toMatchObject({ skipHost: false, skipBase: true, force: false });
    expect(parsePrepareFlags(["--force"])).toMatchObject({ skipHost: false, skipBase: false, force: true });
    expect(parsePrepareFlags(["--skip-host", "--skip-base", "--force"])).toEqual({
      skipHost: true,
      skipBase: true,
      force: true,
      unknown: [],
    });
  });

  it("--force no anula un --skip-*", () => {
    const flags = parsePrepareFlags(["--skip-host", "--force", "--skip-base"]);
    expect(flags.skipHost).toBe(true);
    expect(flags.skipBase).toBe(true);
    expect(flags.force).toBe(true);
  });

  it("es sensible a mayúsculas y no acepta --skip-host=1 ni --skip-sdk", () => {
    const flags = parsePrepareFlags(["--SKIP-HOST", "--skip-host=1", "--skip-sdk", "--skip-strip", "skip-host"]);
    expect(flags.skipHost).toBe(false);
    expect(flags.skipBase).toBe(false);
    expect(flags.unknown).toEqual(["--SKIP-HOST", "--skip-host=1", "--skip-sdk", "--skip-strip", "skip-host"]);
  });

  it("un typo no activa el skip", () => {
    expect(parsePrepareFlags(["--skip-hosts", "--skip-bases"]).skipHost).toBe(false);
    expect(parsePrepareFlags(["--skip-hosts"]).skipBase).toBe(false);
  });

  it("deduplica flags repetidos", () => {
    expect(parsePrepareFlags(["--skip-host", "--skip-host", "--skip-host"])).toMatchObject({
      skipHost: true,
      skipBase: false,
    });
  });
});

describe("runPrepare respeta --skip-* sin tocar cargo/SDK", () => {
  const triple = "x86_64-unknown-linux-gnu";

  function harness() {
    const calls: { host?: { triple: string; force: boolean }; base?: { platform: string; force: boolean } } = {};
    const logs: string[] = [];
    const deps = {
      rustcHostTriple: () => triple,
      prepareHost: (hostTriple: string, force: boolean) => {
        calls.host = { triple: hostTriple, force };
      },
      prepareBase: (platform: string, force: boolean) => {
        calls.base = { platform, force };
      },
      log: (message: string) => {
        logs.push(message);
      },
    };
    return { calls, logs, deps };
  }

  it("--skip-host no llama prepareHost y sí prepareBase", () => {
    const { calls, logs, deps } = harness();
    runPrepare(["--skip-host"], deps);
    expect(calls.host).toBeUndefined();
    expect(calls.base).toEqual({ platform: "linux64", force: false });
    expect(logs.join("\n")).toMatch(/Omitiendo host \(--skip-host\)/);
    expect(logs.join("\n")).not.toMatch(/Omitiendo base/);
  });

  it("--skip-base no llama prepareBase y sí prepareHost", () => {
    const { calls, logs, deps } = harness();
    runPrepare(["--skip-base"], deps);
    expect(calls.base).toBeUndefined();
    expect(calls.host).toEqual({ triple, force: false });
    expect(logs.join("\n")).toMatch(/Omitiendo base \(--skip-base\)/);
  });

  it("--skip-host --skip-base no llama a ninguno (ni con --force)", () => {
    const { calls, logs, deps } = harness();
    runPrepare(["--skip-host", "--skip-base", "--force"], deps);
    expect(calls.host).toBeUndefined();
    expect(calls.base).toBeUndefined();
    expect(logs.some((line) => line === "Listo.")).toBe(true);
  });

  it("sin skips llama ambos y propaga --force", () => {
    const { calls, deps } = harness();
    runPrepare(["--force"], deps);
    expect(calls.host).toEqual({ triple, force: true });
    expect(calls.base).toEqual({ platform: "linux64", force: true });
  });

  it("--skip-sdk es desconocido y no omite host ni base", () => {
    const { calls, logs, deps } = harness();
    runPrepare(["--skip-sdk"], deps);
    expect(calls.host).toBeDefined();
    expect(calls.base).toBeDefined();
    expect(logs.some((line) => line.includes("flag desconocido --skip-sdk"))).toBe(true);
  });

  it("triple no soportado falla antes de prepareHost/prepareBase", () => {
    const { deps } = harness();
    deps.rustcHostTriple = () => "riscv64-unknown-linux-gnu";
    deps.prepareHost = () => throwIfPrepareCalled("prepareHost");
    deps.prepareBase = () => throwIfPrepareCalled("prepareBase");
    expect(() => runPrepare([], deps)).toThrow(PrepareError);
    expect(() => runPrepare([], deps)).toThrow(/Triple no soportado: riscv64-unknown-linux-gnu/);
  });

  it("--skip-host --force no pasa force a un host que no se corre", () => {
    const { calls, deps } = harness();
    runPrepare(["--skip-host", "--force"], deps);
    expect(calls.host).toBeUndefined();
    expect(calls.base).toEqual({ platform: "linux64", force: true });
  });
});

function bunExecutable(): string {
  const homeBun = path.join(os.homedir(), ".bun", "bin", "bun");
  if (fs.existsSync(homeBun)) return homeBun;
  if (process.execPath.endsWith("bun")) return process.execPath;
  return "bun";
}

describe("CLI --skip-host --skip-base (proceso real, sin SDK)", () => {
  it("sale 0 y no invoca cargo", () => {
    const script = fileURLToPath(new URL("./cef-prepare.ts", import.meta.url));
    const bun = bunExecutable();
    const result = spawnSync(bun, [script, "--skip-host", "--skip-base"], {
      cwd: ROOT,
      encoding: "utf8",
      env: {
        ...process.env,
        PATH: `${path.dirname(bun)}${path.delimiter}${process.env.PATH ?? ""}`,
      },
    });
    expect(result.status, `${result.stdout}\n${result.stderr}`).toBe(0);
    const out = `${result.stdout}\n${result.stderr}`;
    expect(out).toMatch(/Omitiendo host \(--skip-host\)/);
    expect(out).toMatch(/Omitiendo base \(--skip-base\)/);
    expect(out).toMatch(/Listo\./);
    expect(out).not.toMatch(/Compilando cef-host/);
    expect(out).not.toMatch(/Buscando SDK/);
    expect(out).not.toMatch(/spotifycdn|cef-builds/i);
  });
});

describe("sha1Eq / archive vs base.json", () => {
  it("acepta mayúsculas y espacios alrededor", () => {
    expect(sha1Eq(LINUX64_SHA1, LINUX64_SHA1)).toBe(true);
    expect(sha1Eq(LINUX64_SHA1.toUpperCase(), LINUX64_SHA1)).toBe(true);
    expect(sha1Eq(`  ${LINUX64_SHA1}  \n`, LINUX64_SHA1)).toBe(true);
    expect(sha1Eq(LINUX64_SHA1, ` ${LINUX64_SHA1.toUpperCase()} `)).toBe(true);
  });

  it("rechaza vacío, null, distinto y truncado", () => {
    expect(sha1Eq("", LINUX64_SHA1)).toBe(false);
    expect(sha1Eq("   ", LINUX64_SHA1)).toBe(false);
    expect(sha1Eq(null, LINUX64_SHA1)).toBe(false);
    expect(sha1Eq(undefined, LINUX64_SHA1)).toBe(false);
    expect(sha1Eq("9711b86c105fb590da576fe5a829802f1a79d521", LINUX64_SHA1)).toBe(false);
    expect(sha1Eq(LINUX64_SHA1.slice(0, 39), LINUX64_SHA1)).toBe(false);
    expect(sha1Eq("0".repeat(40), LINUX64_SHA1)).toBe(false);
  });

  it("assertArchiveMatches exige name exacto además del sha1", () => {
    expect(() => assertArchiveMatches({ name: LINUX64_NAME, sha1: LINUX64_SHA1 }, platformFile())).not.toThrow();
    expect(() =>
      assertArchiveMatches({ name: LINUX64_NAME, sha1: LINUX64_SHA1.toUpperCase() }, platformFile()),
    ).not.toThrow();

    expect(() => assertArchiveMatches({ name: LINUX64_NAME, sha1: "deadbeef" }, platformFile())).toThrow(PrepareError);
    expect(() =>
      assertArchiveMatches({ name: "otro.tar.bz2", sha1: LINUX64_SHA1 }, platformFile()),
    ).toThrow(/no coinciden/);
    expect(() => assertArchiveMatches({ name: LINUX64_NAME }, platformFile())).toThrow(/\(sin sha1\)/);
    expect(() => assertArchiveMatches({ sha1: LINUX64_SHA1 }, platformFile())).toThrow(/\(sin name\)/);
    expect(() => assertArchiveMatches({}, platformFile())).toThrow(/\(sin name\).*\(sin sha1\)/s);
  });
});

describe("prepareBase: sha1 mismatch no borra un base existente", () => {
  it("sha1 distinto falla antes de rmSync del cef-base", () => {
    const root = tempDir("sha1");
    const paths = fixturePaths(root);
    writeJson(paths.baseJsonPath, fakeBaseJson());
    const versionDir = path.join(paths.sdkRoot, "152.0.6");
    writeJson(path.join(versionDir, "archive.json"), {
      name: LINUX64_NAME,
      sha1: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    });
    fs.mkdirSync(paths.cefBaseDir, { recursive: true });
    const canary = path.join(paths.cefBaseDir, "CANARY");
    fs.writeFileSync(canary, "keep-me");

    expect(() => prepareBase("linux64", true, paths)).toThrow(PrepareError);
    expect(() => prepareBase("linux64", true, paths)).toThrow(/no coinciden/);
    expect(fs.existsSync(canary)).toBe(true);
    expect(fs.readFileSync(canary, "utf8")).toBe("keep-me");
  });

  it("name distinto con el mismo sha1 también falla y conserva el dest", () => {
    const root = tempDir("name");
    const paths = fixturePaths(root);
    writeJson(paths.baseJsonPath, fakeBaseJson());
    writeJson(path.join(paths.sdkRoot, "152.0.6", "archive.json"), {
      name: "cef_binary_wrong_linux64_minimal.tar.bz2",
      sha1: LINUX64_SHA1,
    });
    fs.mkdirSync(paths.cefBaseDir, { recursive: true });
    fs.writeFileSync(path.join(paths.cefBaseDir, "manifest.json"), "{}\n");

    expect(() => prepareBase("linux64", true, paths)).toThrow(/no coinciden/);
    expect(fs.existsSync(path.join(paths.cefBaseDir, "manifest.json"))).toBe(true);
  });

  it("sha1 en mayúsculas en archive.json coincide y sigue (falla después, en headers, no por hash)", () => {
    const root = tempDir("sha1-upper");
    const paths = fixturePaths(root);
    writeJson(paths.baseJsonPath, fakeBaseJson());
    writeJson(path.join(paths.sdkRoot, "152.0.6", "archive.json"), {
      name: LINUX64_NAME,
      sha1: LINUX64_SHA1.toUpperCase(),
    });
    expect(() => prepareBase("linux64", true, paths)).toThrow(/Faltan include\/cef_api_versions\.h/);
  });
});

describe("SDK ambigua / ausente", () => {
  it("sin .cef-sdk no descarga: falla pidiendo compilar cef-host", () => {
    const missing = path.join(tempDir("nosdk"), "missing-sdk");
    expect(() => findVersionDir(missing, "152.0.6")).toThrow(PrepareError);
    expect(() => findVersionDir(missing, "152.0.6")).toThrow(/No existe el SDK de CEF/);
    expect(() => findVersionDir(missing, "152.0.6")).toThrow(/Compila cef-host una vez/);
  });

  it("dos prefix+ son ambiguos", () => {
    const sdk = tempDir("amb-ver");
    fs.mkdirSync(path.join(sdk, "152.0.6+gaaaa"));
    fs.mkdirSync(path.join(sdk, "152.0.6+gbbbb"));
    expect(() => findVersionDir(sdk, "152.0.6")).toThrow(/Varios directorios de SDK coinciden con 152\.0\.6/);
    try {
      findVersionDir(sdk, "152.0.6");
    } catch (error) {
      expect(error).toBeInstanceOf(PrepareError);
      const message = (error as Error).message;
      expect(message).toContain(path.join(sdk, "152.0.6+gaaaa"));
      expect(message).toContain(path.join(sdk, "152.0.6+gbbbb"));
    }
  });

  it("exacto 152.0.6 gana aunque existan 152.0.6+hermanos", () => {
    const sdk = tempDir("exact");
    const exact = path.join(sdk, "152.0.6");
    fs.mkdirSync(exact);
    fs.mkdirSync(path.join(sdk, "152.0.6+g708dc14"));
    expect(findVersionDir(sdk, "152.0.6")).toBe(exact);
  });

  it("152.0.60 no casa con el prefijo 152.0.6", () => {
    const sdk = tempDir("prefix");
    fs.mkdirSync(path.join(sdk, "152.0.60"));
    expect(() => findVersionDir(sdk, "152.0.6")).toThrow(/No hay un directorio de SDK cuya versión coincida/);
  });

  it("un archivo con el nombre de la versión no cuenta", () => {
    const sdk = tempDir("file");
    fs.writeFileSync(path.join(sdk, "152.0.6"), "not-a-dir");
    fs.writeFileSync(path.join(sdk, "152.0.6+gfoo"), "neither");
    expect(() => findVersionDir(sdk, "152.0.6")).toThrow(/No hay un directorio/);
  });

  it("un solo 152.0.6+commit es inequívoco", () => {
    const sdk = tempDir("one");
    const only = path.join(sdk, "152.0.6+g708dc14");
    fs.mkdirSync(only);
    expect(findVersionDir(sdk, "152.0.6")).toBe(only);
  });

  it("152.0.6-beta y 152.0.6+g son ambiguos", () => {
    const sdk = tempDir("plus-minus");
    fs.mkdirSync(path.join(sdk, "152.0.6-beta"));
    fs.mkdirSync(path.join(sdk, "152.0.6+g1"));
    expect(() => findVersionDir(sdk, "152.0.6")).toThrow(/Varios directorios/);
  });

  it("dos archive.json hermanos son un slot ambiguo", () => {
    const versionDir = tempDir("amb-slot");
    writeJson(path.join(versionDir, "linux64", "archive.json"), { name: "a", sha1: "1" });
    writeJson(path.join(versionDir, "linux64-debug", "archive.json"), { name: "b", sha1: "2" });
    expect(() => findSdkSlot(versionDir)).toThrow(/Varios slots de SDK con archive\.json/);
  });

  it("archive.json en la raíz del versionDir no es ambiguo aunque haya hijos", () => {
    const versionDir = tempDir("root-slot");
    writeJson(path.join(versionDir, "archive.json"), { name: LINUX64_NAME, sha1: LINUX64_SHA1 });
    writeJson(path.join(versionDir, "extra", "archive.json"), { name: "other" });
    expect(findSdkSlot(versionDir)).toBe(versionDir);
  });

  it("sin archive.json falla", () => {
    const versionDir = tempDir("empty-slot");
    fs.mkdirSync(path.join(versionDir, "Release"), { recursive: true });
    expect(() => findSdkSlot(versionDir)).toThrow(/No se encontró archive\.json/);
  });

  it("prepareBase con SDK ambigua no toca cef-base", () => {
    const root = tempDir("prep-amb");
    const paths = fixturePaths(root);
    writeJson(paths.baseJsonPath, fakeBaseJson());
    writeJson(path.join(paths.sdkRoot, "152.0.6+ga", "archive.json"), { name: LINUX64_NAME, sha1: LINUX64_SHA1 });
    writeJson(path.join(paths.sdkRoot, "152.0.6+gb", "archive.json"), { name: LINUX64_NAME, sha1: LINUX64_SHA1 });
    fs.mkdirSync(paths.cefBaseDir, { recursive: true });
    fs.writeFileSync(path.join(paths.cefBaseDir, "CANARY"), "stay");

    expect(() => prepareBase("linux64", true, paths)).toThrow(/Varios directorios de SDK/);
    expect(fs.readFileSync(path.join(paths.cefBaseDir, "CANARY"), "utf8")).toBe("stay");
  });
});

describe("baseAlreadyPrepared y --force", () => {
  it("si el base ya está, !force no mira la SDK (aunque no exista)", () => {
    const root = tempDir("ready");
    const paths = fixturePaths(root);
    writeJson(paths.baseJsonPath, fakeBaseJson());
    fs.mkdirSync(paths.cefBaseDir, { recursive: true });
    fs.writeFileSync(path.join(paths.cefBaseDir, "libcef.so"), "abc");
    writeJson(path.join(paths.cefBaseDir, "manifest.json"), {
      cefVersion: "152.0.6+g708dc14+chromium-152.0.7977.83",
      files: [{ path: "libcef.so", size: 3 }],
    });
    expect(baseAlreadyPrepared(paths.cefBaseDir, "152.0.6+g708dc14+chromium-152.0.7977.83")).toBe(true);
    expect(() => prepareBase("linux64", false, paths)).not.toThrow();
    expect(fs.existsSync(paths.sdkRoot)).toBe(false);
  });

  it("--force ignora el base ya preparado y exige SDK", () => {
    const root = tempDir("force");
    const paths = fixturePaths(root);
    writeJson(paths.baseJsonPath, fakeBaseJson());
    fs.mkdirSync(paths.cefBaseDir, { recursive: true });
    fs.writeFileSync(path.join(paths.cefBaseDir, "libcef.so"), "abc");
    writeJson(path.join(paths.cefBaseDir, "manifest.json"), {
      cefVersion: "152.0.6+g708dc14+chromium-152.0.7977.83",
      files: [{ path: "libcef.so", size: 3 }],
    });
    expect(() => prepareBase("linux64", true, paths)).toThrow(/No existe el SDK de CEF/);
    expect(fs.existsSync(path.join(paths.cefBaseDir, "libcef.so"))).toBe(true);
  });

  it("manifest con size mentiroso no cuenta como preparado", () => {
    const root = tempDir("stale");
    const baseDir = path.join(root, "cef-base");
    fs.mkdirSync(baseDir, { recursive: true });
    fs.writeFileSync(path.join(baseDir, "libcef.so"), "ab");
    writeJson(path.join(baseDir, "manifest.json"), {
      cefVersion: "152.0.6+g708dc14+chromium-152.0.7977.83",
      files: [{ path: "libcef.so", size: 99 }],
    });
    expect(baseAlreadyPrepared(baseDir, "152.0.6+g708dc14+chromium-152.0.7977.83")).toBe(false);
  });
});

describe("helpers de versión / plataforma", () => {
  it("majorMinorPatch corta en el primer +", () => {
    expect(majorMinorPatch("152.0.6+g708dc14+chromium-152.0.7977.83")).toBe("152.0.6");
    expect(majorMinorPatch("152.0.6")).toBe("152.0.6");
    expect(majorMinorPatch("")).toBe("");
  });

  it("mapea triples conocidos y rechaza el resto", () => {
    expect(platformOfTriple("x86_64-unknown-linux-gnu")).toBe("linux64");
    expect(platformOfTriple("aarch64-unknown-linux-gnu")).toBe("linuxarm64");
    expect(platformOfTriple("x86_64-pc-windows-msvc")).toBeUndefined();
    expect(platformOfTriple("x86_64-apple-darwin")).toBeUndefined();
    expect(platformOfTriple("aarch64-apple-darwin")).toBeUndefined();
    expect(platformOfTriple("wasm32-unknown-unknown")).toBeUndefined();
  });
});
