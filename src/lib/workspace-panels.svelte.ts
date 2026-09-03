import { appConfig, type LayoutSettings } from "$lib/app-config.svelte";
import {
  clampPanelSize,
  DEFAULT_TREE_WIDTH,
  MIN_TREE_WIDTH,
  treeReserve,
} from "$lib/panel-resize";
import type { TerminalDock } from "$lib/terminal-dock";
import { terminal } from "$lib/terminal.svelte";

/**
 * Geometry of the two optional regions of the IDE view. The editor and the footer
 * are always there; the tree and the terminal are the ones the user moves around.
 */
class WorkspacePanels {
  treeVisible = $state(true);
  treeWidth = $state(DEFAULT_TREE_WIDTH);

  #hydrated = false;

  /** Reads the stored layout once, the first time the config lands. */
  hydrate(layout: LayoutSettings): void {
    if (this.#hydrated) return;
    this.#hydrated = true;

    this.treeVisible = layout.treeVisible;
    terminal.hydrate(layout, this.treeSpace);
    this.setTreeWidth(layout.treeWidth, window.innerWidth);
  }

  /** Width the terminal must work around: zero when the tree is hidden. */
  get treeSpace(): number {
    return this.treeVisible ? this.treeWidth : 0;
  }

  setTreeWidth(pixels: number, viewport: number): void {
    const rightDock = terminal.peeking && terminal.dock === "right" ? terminal.rightSize : 0;
    this.treeWidth = clampPanelSize(pixels, MIN_TREE_WIDTH, viewport, treeReserve(rightDock));
  }

  /**
   * Keeps both panels inside the window. The tree yields first: when room runs out
   * it is usually because the terminal was just asked for, or the window shrank.
   */
  fit(width: number, height: number): void {
    this.setTreeWidth(this.treeWidth, width);
    terminal.fit(width, height, this.treeSpace);
  }

  toggleTree(): void {
    this.treeVisible = !this.treeVisible;
    if (this.treeVisible) this.fit(window.innerWidth, window.innerHeight);
    this.persist();
  }

  showTree(): void {
    if (this.treeVisible) return;
    this.toggleTree();
  }

  /** Toggling the terminal can move its dock, so the pair is stored together. */
  toggleTerminal(dock: TerminalDock): void {
    terminal.toggle(dock);
    // A terminal that just appeared on the right may need room the tree is holding.
    this.fit(window.innerWidth, window.innerHeight);
    this.persist();
  }

  /** Called when a resize drag ends, so a drag writes the config once. */
  commitResize(): void {
    this.persist();
  }

  persist(): void {
    void appConfig.saveLayout(this.snapshot());
  }

  snapshot(): LayoutSettings {
    return {
      treeWidth: this.treeWidth,
      treeVisible: this.treeVisible,
      terminalBottom: terminal.bottomSize,
      terminalRight: terminal.rightSize,
      terminalDock: terminal.dock,
    };
  }
}

export const panels = new WorkspacePanels();
