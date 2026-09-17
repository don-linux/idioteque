import path from "node:path";
import { describe, expect, it } from "vitest";

import {
  APPIMAGE_OVERRIDE_CONFIG,
  appImageArch,
  archOfTriple,
  artifactNames,
  buildEnv,
  buildTmpDir,
  extractBundles,
  extractTarget,
  hasFlag,
  planBuild,
  profileDir,
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
