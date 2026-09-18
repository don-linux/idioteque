// svelte-check has no @types/node; the test runner provides node:fs at runtime.
// @ts-expect-error Node built-in used only in this guard test.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  baseMatchesCrate,
  cefVersionFromCargoLock,
  cefVersionsFromCargoLock,
  chromiumFromCefVersion,
  pinMatchesLock,
} from "./cef-base";

const SAMPLE_LOCK = `[[package]]
name = "cef-dll-sys"
version = "152.3.0+999.0.0"

[[package]]
name = "cef"
version = "152.3.0+152.0.6"
source = "registry+https://github.com/rust-lang/crates.io-index"
`;

describe("cefVersionFromCargoLock", () => {
  it("returns the CEF version after the + in the cef package block", () => {
    expect(cefVersionFromCargoLock(SAMPLE_LOCK)).toBe("152.0.6");
  });

  it("does not treat cef-dll-sys as the cef crate", () => {
    const lock = `[[package]]
name = "cef-dll-sys"
version = "152.3.0+9.9.9"
`;
    expect(cefVersionFromCargoLock(lock)).toBeNull();
    expect(cefVersionsFromCargoLock(lock)).toEqual([]);
  });

  it("does not treat icef or cef-extra as the cef crate", () => {
    const lock = `name = "icef"
version = "1.0.0+9.9.9"

name = "cef-extra"
version = "1.0.0+8.8.8"

dependencies = [
 "cef",
]
`;
    expect(cefVersionFromCargoLock(lock)).toBeNull();
  });

  it("returns null when the crate version has no + suffix", () => {
    const lock = `name = "cef"
version = "152.3.0"
`;
    expect(cefVersionFromCargoLock(lock)).toBeNull();
  });

  it("returns null when the + suffix is empty", () => {
    const lock = `name = "cef"
version = "152.3.0+"
`;
    expect(cefVersionFromCargoLock(lock)).toBeNull();
  });

  it("returns null for an empty lockfile", () => {
    expect(cefVersionFromCargoLock("")).toBeNull();
  });

  it("reads CRLF Cargo.lock the same as LF", () => {
    const crlf = SAMPLE_LOCK.split("\n").join("\r\n");
    expect(cefVersionFromCargoLock(crlf)).toBe("152.0.6");
  });

  it("strips a UTF-8 BOM before parsing", () => {
    expect(cefVersionFromCargoLock(`\uFEFF${SAMPLE_LOCK}`)).toBe("152.0.6");
  });

  it("accepts two cef packages that share the same + suffix", () => {
    const lock = `name = "cef"
version = "152.3.0+152.0.6"

name = "cef"
version = "152.4.0+152.0.6"
`;
    expect(cefVersionsFromCargoLock(lock)).toEqual(["152.0.6", "152.0.6"]);
    expect(cefVersionFromCargoLock(lock)).toBe("152.0.6");
  });

  it("returns null when two cef packages disagree on the + suffix", () => {
    const lock = `name = "cef"
version = "152.3.0+152.0.6"

name = "cef"
version = "141.0.0+141.0.1"
`;
    expect(cefVersionFromCargoLock(lock)).toBeNull();
    expect(new Set(cefVersionsFromCargoLock(lock))).toEqual(new Set(["152.0.6", "141.0.1"]));
  });

  it("requires name and version on consecutive lines (Cargo.lock order)", () => {
    const lock = `name = "cef"
source = "registry+https://github.com/rust-lang/crates.io-index"
version = "152.3.0+152.0.6"
`;
    expect(cefVersionFromCargoLock(lock)).toBeNull();
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

  it("does not treat 152.0.6 as a prefix of 152.0.60", () => {
    expect(baseMatchesCrate("152.0.60+gdead+chromium-1.0.0", "152.0.6")).toBe(false);
  });

  it("does not treat a shorter crate triple as a prefix of a longer one", () => {
    expect(baseMatchesCrate("152.0.6+g708dc14+chromium-152.0.7977.83", "152.0")).toBe(false);
    expect(baseMatchesCrate("152.0.6+g708dc14+chromium-152.0.7977.83", "152")).toBe(false);
  });
});

describe("chromiumFromCefVersion / pinMatchesLock", () => {
  it("reads the chromium- suffix of a Spotify-style CEF version", () => {
    expect(chromiumFromCefVersion("152.0.6+g708dc14+chromium-152.0.7977.83")).toBe(
      "152.0.7977.83",
    );
  });

  it("returns null without a chromium- marker or with an empty suffix", () => {
    expect(chromiumFromCefVersion("152.0.6+g708dc14")).toBeNull();
    expect(chromiumFromCefVersion("152.0.6+g708dc14+chromium-")).toBeNull();
  });

  it("requires crate prefix and chromiumVersion to agree with cefVersion", () => {
    const base = {
      cefVersion: "152.0.6+g708dc14+chromium-152.0.7977.83",
      chromiumVersion: "152.0.7977.83",
    };
    expect(pinMatchesLock(base, "152.0.6")).toBe(true);
    expect(pinMatchesLock({ ...base, chromiumVersion: "152.0.0.0" }, "152.0.6")).toBe(false);
    expect(pinMatchesLock(base, "152.0.7")).toBe(false);
  });
});

describe("base.json vs Cargo.lock", () => {
  const lockText = readFileSync("src-tauri/Cargo.lock", "utf8");
  const base = JSON.parse(readFileSync("src-tauri/cef/base.json", "utf8")) as {
    cefVersion: string;
    chromiumVersion: string;
    hostApiVersion: number;
    apiVersionMin: number;
    indexUrl: string;
    downloadBaseUrl: string;
    files: Record<string, { name: string; sha1: string; size: number }>;
  };

  it("keeps the bundled CEF base on the same version as the unique cef crate", () => {
    const crateCefVersion = cefVersionFromCargoLock(lockText);
    expect(crateCefVersion).toBe("152.0.6");
    expect(base.cefVersion.startsWith("152.0.6+")).toBe(true);
    expect(baseMatchesCrate(base.cefVersion, crateCefVersion ?? "")).toBe(true);
    expect(pinMatchesLock(base, crateCefVersion ?? "")).toBe(true);
    expect(cefVersionsFromCargoLock(lockText)).toEqual(["152.0.6"]);
  });

  it("keeps chromiumVersion equal to the chromium- suffix, not a distro package", () => {
    expect(base.chromiumVersion).toBe("152.0.7977.83");
    expect(chromiumFromCefVersion(base.cefVersion)).toBe(base.chromiumVersion);
    expect(base.chromiumVersion).not.toMatch(/ubuntu|debian|fedora|rhel/i);
  });

  it("pins hostApiVersion to CEF major × 100 (bindings), not an apt epoch", () => {
    const major = Number(base.cefVersion.split(".")[0]);
    expect(base.hostApiVersion).toBe(15200);
    expect(base.hostApiVersion).toBe(major * 100);
    expect(base.apiVersionMin).toBeLessThanOrEqual(base.hostApiVersion);
  });

  it("downloads from the official Spotify CEF index, not a distro mirror", () => {
    expect(base.indexUrl).toBe("https://cef-builds.spotifycdn.com/index.json");
    expect(base.downloadBaseUrl).toBe("https://cef-builds.spotifycdn.com/");
    expect(base.indexUrl).not.toMatch(/ubuntu|debian|launchpad|fedoraproject|rpmfind/i);
  });

  it("names every supported Linux CEF platform tarball after the pin (not apt names)", () => {
    const platforms = ["linux64", "linuxarm64"];
    expect(Object.keys(base.files).sort()).toEqual([...platforms].sort());
    for (const platform of platforms) {
      const file = base.files[platform];
      expect(file.name).toBe(`cef_binary_${base.cefVersion}_${platform}_minimal.tar.bz2`);
      expect(file.sha1).toMatch(/^[0-9a-f]{40}$/);
      expect(file.size).toBeGreaterThan(0);
      expect(file.name).not.toMatch(/ubuntu|jammy|noble|t64/);
    }
  });
});
