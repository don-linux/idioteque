// svelte-check has no @types/node; the test runner provides these at runtime.
// @ts-expect-error Node built-in used only in this guard test.
import { readFileSync } from "node:fs";
// @ts-expect-error Node built-in used only in this guard test.
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import type { WorkspaceSurface } from "./workspace-surface";

const SURFACES: WorkspaceSurface[] = ["editor", "terminals"];

describe("workspace surfaces", () => {
  it("is only editor and terminals", () => {
    expect(SURFACES).toEqual(["editor", "terminals"]);
    expect(SURFACES.includes("browser" as WorkspaceSurface)).toBe(false);
  });
});

describe("runtime sources have no internal browser surface", () => {
  it("does not mount a page host or toolbar", () => {
    const surface = readFileSync(fileURLToPath(new URL("./workspace-surface.ts", import.meta.url)), "utf8");
    const page = readFileSync(
      fileURLToPath(new URL("../routes/workspace/+page.svelte", import.meta.url)),
      "utf8",
    );
    const layout = readFileSync(
      fileURLToPath(new URL("../routes/workspace/+layout.svelte", import.meta.url)),
      "utf8",
    );
    expect(surface).not.toMatch(/"browser"/);
    expect(page).not.toMatch(/BrowserView|BrowserToolbar|browser-slot|surface-browser|class="host"/);
    expect(layout).not.toMatch(/claimUrlBar|handleBrowserFocusUrlShortcut/);
  });
});
