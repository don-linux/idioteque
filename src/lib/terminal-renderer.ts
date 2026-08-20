export type TerminalRendererKind = "webgl" | "dom";

export interface WebglAddonLike {
  dispose(): void;
  onContextLoss(listener: () => void): { dispose(): void };
}

export interface TerminalRendererHost {
  loadAddon(addon: WebglAddonLike): void;
}

export interface TerminalRendererHandle {
  kind: TerminalRendererKind;
  dispose(): void;
}

export function attachTerminalRenderer(
  terminal: TerminalRendererHost,
  createWebgl: () => WebglAddonLike,
): TerminalRendererHandle {
  let addon: WebglAddonLike;

  try {
    addon = createWebgl();
  } catch {
    return idleDomRenderer();
  }

  try {
    const loss = addon.onContextLoss(() => {
      addon.dispose();
    });

    terminal.loadAddon(addon);

    return {
      kind: "webgl",
      dispose() {
        loss.dispose();
        addon.dispose();
      },
    };
  } catch {
    addon.dispose();
    return idleDomRenderer();
  }
}

function idleDomRenderer(): TerminalRendererHandle {
  return {
    kind: "dom",
    dispose() {},
  };
}
