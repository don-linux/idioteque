// svelte-check has no @types/node; the test runner provides node:fs at runtime.
// @ts-expect-error Node built-in used only in this guard test.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { baseMatchesCrate, cefVersionFromCargoLock } from "./cef-base";

describe("cefVersionFromCargoLock", () => {
  it("returns the CEF version after the + in the cef package block", () => {
    const lock = `[[package]]
name = "cef-dll-sys"
version = "152.3.0+999.0.0"

[[package]]
name = "cef"
version = "152.3.0+152.0.6"
source = "registry+https://github.com/rust-lang/crates.io-index"
`;
    expect(cefVersionFromCargoLock(lock)).toBe("152.0.6");
  });

  it("does not treat cef-dll-sys as the cef crate", () => {
    const lock = `[[package]]
name = "cef-dll-sys"
version = "152.3.0+9.9.9"
`;
    expect(cefVersionFromCargoLock(lock)).toBeNull();
  });

  it("returns null when the crate version has no + suffix", () => {
    const lock = `name = "cef"
version = "152.3.0"
`;
    expect(cefVersionFromCargoLock(lock)).toBeNull();
  });

  it("returns null for an empty lockfile", () => {
    expect(cefVersionFromCargoLock("")).toBeNull();
  });
});

describe("baseMatchesCrate", () => {
  it("accepts a base version that starts with the crate CEF version and +", () => {
    expect(
      baseMatchesCrate("152.0.6+g708dc14+chromium-152.0.7977.83", "152.0.6"),
    ).toBe(true);
  });

  it("rejects a different patch or a crate version without a following +", () => {
    expect(baseMatchesCrate("152.0.7+g708dc14+chromium-152.0.7977.83", "152.0.6")).toBe(false);
    expect(baseMatchesCrate("152.0.6", "152.0.6")).toBe(false);
    expect(baseMatchesCrate("", "152.0.6")).toBe(false);
    expect(baseMatchesCrate("152.0.6+abc", "")).toBe(false);
  });
});

describe("base.json vs Cargo.lock", () => {
  it("keeps the bundled CEF base on the same version as the cef crate", () => {
    const lockText = readFileSync("src-tauri/Cargo.lock", "utf8");
    const base = JSON.parse(readFileSync("src-tauri/cef/base.json", "utf8")) as {
      cefVersion: string;
    };
    const crateCefVersion = cefVersionFromCargoLock(lockText);
    expect(crateCefVersion).toBe("152.0.6");
    expect(base.cefVersion.startsWith("152.0.6+")).toBe(true);
    expect(baseMatchesCrate(base.cefVersion, crateCefVersion ?? "")).toBe(true);
  });
});
