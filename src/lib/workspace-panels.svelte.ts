import { appConfig, type LayoutSettings } from "$lib/app-config.svelte";
import { clampPanelSize, DEFAULT_TREE_WIDTH, MIN_TREE_WIDTH } from "$lib/panel-resize";
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
    this.treeWidth = clampPanelSize(layout.treeWidth, MIN_TREE_WIDTH, window.innerWidth);
    terminal.hydrate(layout);
  }

  setTreeWidth(pixels: number, viewport: number): void {
    this.treeWidth = clampPanelSize(pixels, MIN_TREE_WIDTH, viewport);
  }

  toggleTree(): void {
    this.treeVisible = !this.treeVisible;
    this.persist();
  }

  showTree(): void {
    if (this.treeVisible) return;
    this.treeVisible = true;
    this.persist();
  }

  /** Toggling the terminal can move its dock, so the pair is stored together. */
  toggleTerminal(dock: TerminalDock): void {
    terminal.toggle(dock);
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
