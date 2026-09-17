import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { compile } from "svelte/compiler";
import { afterEach, describe, expect, it, vi } from "vitest";
import { displayUrl } from "$lib/browser-url";

const TOOLBAR_PATH = fileURLToPath(new URL("./BrowserToolbar.svelte", import.meta.url));

function compileToolbar(): string {
  const source = readFileSync(TOOLBAR_PATH, "utf8");
  const result = compile(source, {
    filename: "BrowserToolbar.svelte",
    css: "external",
    generate: "client",
    dev: false,
  });
  expect(result.warnings, result.warnings.map((warning) => warning.message).join("\n")).toEqual([]);
  return result.js.code;
}

function extractFunction(source: string, name: string): string {
  const header = new RegExp(`function ${name}\\(`);
  const match = header.exec(source);
  if (!match) throw new Error(`missing function ${name}`);

  let brace = match.index + match[0].length - 1;
  while (brace < source.length && source[brace] !== "{") brace += 1;

  let depth = 0;
  for (let i = brace; i < source.length; i += 1) {
    if (source[i] === "{") depth += 1;
    else if (source[i] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(match.index, i + 1);
    }
  }
  throw new Error(`unclosed function ${name}`);
}

function disabledExpressions(js: string): string[] {
  return [...js.matchAll(/\.disabled = ([^\n;]+)/g)].map((match) =>
    match[1].replace(/\)+$/, "").trim(),
  );
}

function evalDisabled(expr: string, browser: object): boolean {
  return Boolean(new Function("browser", `return (${expr});`)(browser));
}

class FakeInput {
  blur = vi.fn();
}

function loadHandlers(js: string) {
  const src = ["onToolbarFocusIn", "onUrlKeydown"]
    .map((name) => extractFunction(js, name))
    .join("\n");

  return (browser: object) =>
    new Function(
      "browser",
      "displayUrl",
      "HTMLInputElement",
      "shouldClaimAppFocus",
      `${src}\nreturn { onToolbarFocusIn, onUrlKeydown };`,
    )(
      browser,
      displayUrl,
      FakeInput,
      (owner: string) => owner !== "app",
    ) as {
      onToolbarFocusIn: () => void;
      onUrlKeydown: (event: {
        key: string;
        currentTarget: unknown;
        preventDefault: () => void;
      }) => void;
    };
}

function fakeBrowser(overrides: Record<string, unknown> = {}) {
  return {
    alive: true,
    loading: false,
    canGoBack: false,
    canGoForward: false,
    error: null as string | null,
    url: "https://example.com/page/",
    inputUrl: "example.com/other",
    focusUrlRequested: 0,
    focusOwner: "browser",
    focusApp: vi.fn().mockImplementation(async function (this: { focusOwner: string }) {
      this.focusOwner = "app";
    }),
    focus: vi.fn().mockResolvedValue(undefined),
    navigate: vi.fn().mockResolvedValue(undefined),
    back: vi.fn(),
    forward: vi.fn(),
    stop: vi.fn(),
    reload: vi.fn(),
    devtools: vi.fn(),
    leave: vi.fn(),
    respawn: vi.fn(),
    ...overrides,
  };
}

function keyEvent(key: string, currentTarget: unknown) {
  return {
    key,
    currentTarget,
    preventDefault: vi.fn(),
  };
}

describe("BrowserToolbar", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("reclaims X11 via one browser_focus_app on toolbar focusin", () => {
    const js = compileToolbar();

    expect(js).toMatch(/class="toolbar/);
    expect(js).toMatch(/data-browser-toolbar/);
    expect(js).toMatch(/data-browser-url/);
    expect(js).toMatch(/data-browser-chrome-last/);
    expect(js).toMatch(/var div = root_3\(\)/);
    expect(js).toMatch(/\$\.delegated\('focusin',\s*div,\s*onToolbarFocusIn\)/);
    expect(js).not.toMatch(/pointerdown/);
    expect(js).not.toMatch(/\$\.event\('focus',\s*input_1/);
    expect(js).not.toMatch(/\$\.delegated\('focus',\s*input_1/);

    const focusin = extractFunction(js, "onToolbarFocusIn");
    expect(focusin).toMatch(/shouldClaimAppFocus\(browser\.focusOwner\)/);
    expect(focusin).toMatch(/browser\.focusApp\(\)/);
    expect(focusin).not.toMatch(/browser\.focus\(/);
    expect(focusin).not.toMatch(/alive/);
    expect(focusin).not.toMatch(/invoke\(/);

    const browser = fakeBrowser({ alive: false, error: "El navegador necesita X11" });
    const { onToolbarFocusIn } = loadHandlers(js)(browser);
    onToolbarFocusIn();
    onToolbarFocusIn();

    expect(browser.focusApp).toHaveBeenCalledTimes(1);
    expect(browser.focus).not.toHaveBeenCalled();
    expect(browser.navigate).not.toHaveBeenCalled();
    expect(browser.leave).not.toHaveBeenCalled();
  });

  it("still reclaims app focus when the host is alive, booting, or already focused", () => {
    const js = compileToolbar();
    const handlers = loadHandlers(js);

    for (const state of [
      { alive: true, error: null, focusOwner: "browser" },
      { alive: false, error: null, focusOwner: "browser" },
      { alive: false, error: "El sandbox de Chromium no está disponible", focusOwner: "browser" },
    ]) {
      const browser = fakeBrowser(state);
      handlers(browser).onToolbarFocusIn();
      expect(browser.focusApp, JSON.stringify(state)).toHaveBeenCalledTimes(1);
      expect(browser.focus, JSON.stringify(state)).not.toHaveBeenCalled();
    }
  });

  it("does not send a second browser_focus_app when the app already owns the keyboard", () => {
    const js = compileToolbar();
    const browser = fakeBrowser({ focusOwner: "app" });
    loadHandlers(js)(browser).onToolbarFocusIn();
    expect(browser.focusApp).not.toHaveBeenCalled();
    expect(browser.focus).not.toHaveBeenCalled();
  });

  it("reclaims X11 before focusing the URL on Ctrl+L, even if the host is dead", () => {
    const js = compileToolbar();
    const attach = extractFunction(js, "attachUrl");

    expect(attach).toMatch(/browser\.focusUrlRequested/);
    expect(attach).toMatch(/surface\.current !== "browser"/);
    expect(attach.indexOf("browser.focusApp()")).toBeGreaterThan(-1);
    expect(attach.indexOf("browser.focusApp()")).toBeLessThan(attach.indexOf("node.focus()"));
    expect(attach.indexOf("node.focus()")).toBeLessThan(attach.indexOf("node.select()"));
    expect(attach).not.toMatch(/alive/);
    expect(attach).not.toMatch(/browser\.focus\(/);
  });

  it("disables chrome nav when the host is not alive", () => {
    const js = compileToolbar();
    const exprs = disabledExpressions(js);
    expect(exprs).toHaveLength(5);

    const back = exprs.find((expr) => expr.includes("canGoBack"));
    const forward = exprs.find((expr) => expr.includes("canGoForward"));
    const aliveOnly = exprs.filter(
      (expr) => !expr.includes("canGoBack") && !expr.includes("canGoForward"),
    );

    expect(back).toBeTruthy();
    expect(forward).toBeTruthy();
    expect(aliveOnly).toHaveLength(3);

    const historyStates = [
      { alive: false, canGoBack: true, canGoForward: true, expectDisabled: true },
      { alive: false, canGoBack: false, canGoForward: false, expectDisabled: true },
      { alive: true, canGoBack: true, canGoForward: true, expectDisabled: false },
      { alive: true, canGoBack: false, canGoForward: false, expectDisabled: true },
    ];

    for (const state of historyStates) {
      expect(evalDisabled(back!, state), `back ${JSON.stringify(state)}`).toBe(
        state.expectDisabled || !state.canGoBack,
      );
      expect(evalDisabled(forward!, state), `forward ${JSON.stringify(state)}`).toBe(
        state.expectDisabled || !state.canGoForward,
      );
    }

    // !alive + canGoBack must still disable: a missing alive check is the bug.
    expect(evalDisabled(back!, { alive: false, canGoBack: true })).toBe(true);
    expect(evalDisabled(forward!, { alive: false, canGoForward: true })).toBe(true);

    for (const expr of aliveOnly) {
      expect(evalDisabled(expr, { alive: false }), expr).toBe(true);
      expect(evalDisabled(expr, { alive: true }), expr).toBe(false);
    }
  });

  it("keeps Cerrar and Reintentar enabled when the host is dead", () => {
    const js = compileToolbar();

    expect(js).toMatch(/class="retry/);
    expect(js).toMatch(/Reintentar/);
    expect(js).toMatch(/\$\.delegated\('click',\s*button_6,\s*\(\) => void browser\.respawn\(\)\)/);
    expect(js).toMatch(/\$\.delegated\('click',\s*button_5,\s*\(\) => browser\.leave\(\)\)/);
    expect(js).toMatch(/if \(browser\.error\)/);

    const retryBlock = js.slice(js.indexOf("consequent_1"), js.indexOf("if (browser.error)"));
    expect(retryBlock).not.toMatch(/\.disabled\s*=/);

    const closeHandler = js.match(
      /\$\.delegated\('click',\s*button_5,\s*\(\) => browser\.leave\(\)\)/,
    );
    expect(closeHandler).toBeTruthy();
    expect(js).not.toMatch(/button_5\.disabled/);
  });

  it("wires stop while loading and reload otherwise, both gated on alive", () => {
    const js = compileToolbar();
    expect(js).toMatch(/if \(browser\.loading\) \$\$render\(consequent\)/);
    expect(js).toMatch(/aria-label="Detener"/);
    expect(js).toMatch(/aria-label="Recargar"/);
    expect(js).toMatch(/\$\.delegated\('click',\s*button_2,\s*\(\) => void browser\.stop\(\)\)/);
    expect(js).toMatch(/\$\.delegated\('click',\s*button_3,\s*\(\) => void browser\.reload\(\)\)/);
    expect(js).toMatch(/\$\.delegated\('click',\s*button_4,\s*\(\) => void browser\.devtools\(\)\)/);
    expect(js).toMatch(/\$\.delegated\('click',\s*button,\s*\(\) => void browser\.back\(\)\)/);
    expect(js).toMatch(/\$\.delegated\('click',\s*button_1,\s*\(\) => void browser\.forward\(\)\)/);
  });

  it("navigates on Enter only while the host is alive", () => {
    const js = compileToolbar();
    const handlers = loadHandlers(js);
    const input = new FakeInput();

    const dead = fakeBrowser({ alive: false, inputUrl: "example.com" });
    const deadEvent = keyEvent("Enter", input);
    handlers(dead).onUrlKeydown(deadEvent);
    expect(deadEvent.preventDefault).toHaveBeenCalledTimes(1);
    expect(dead.navigate).not.toHaveBeenCalled();
    expect(dead.focusApp).not.toHaveBeenCalled();

    const live = fakeBrowser({ alive: true, inputUrl: "example.com" });
    const liveEvent = keyEvent("Enter", input);
    handlers(live).onUrlKeydown(liveEvent);
    expect(liveEvent.preventDefault).toHaveBeenCalledTimes(1);
    expect(live.navigate).toHaveBeenCalledTimes(1);
    expect(live.navigate).toHaveBeenCalledWith("example.com");
  });

  it("does not treat a lowercase enter or a non-input target as navigation", () => {
    const js = compileToolbar();
    const { onUrlKeydown } = loadHandlers(js)(fakeBrowser());

    const stray = keyEvent("Enter", { blur: vi.fn() });
    onUrlKeydown(stray);
    expect(stray.preventDefault).not.toHaveBeenCalled();

    const live = fakeBrowser();
    const { onUrlKeydown: keyed } = loadHandlers(js)(live);
    const lower = keyEvent("enter", new FakeInput());
    keyed(lower);
    expect(live.navigate).not.toHaveBeenCalled();
    expect(lower.preventDefault).not.toHaveBeenCalled();
  });

  it("restores the displayed URL on Escape even when the host is dead", () => {
    const js = compileToolbar();
    const browser = fakeBrowser({
      alive: false,
      url: "https://example.com/page/",
      inputUrl: "typed-but-not-committed",
    });
    const input = new FakeInput();
    const event = keyEvent("Escape", input);

    loadHandlers(js)(browser).onUrlKeydown(event);

    expect(event.preventDefault).toHaveBeenCalledTimes(1);
    expect(browser.inputUrl).toBe(displayUrl("https://example.com/page/"));
    expect(browser.inputUrl).toBe("example.com/page");
    expect(input.blur).toHaveBeenCalledTimes(1);
    expect(browser.navigate).not.toHaveBeenCalled();
  });
});
