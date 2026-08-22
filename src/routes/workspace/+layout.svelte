<script lang="ts">
  import type { Snippet } from "svelte";
  import FooterActions from "$lib/components/FooterActions.svelte";
  import FooterTransient from "$lib/components/FooterTransient.svelte";
  import { handleSaveShortcut } from "$lib/save-shortcut";
  import { handleTerminalShortcut, handleTerminalSurfaceShortcut } from "$lib/terminal-dock";
  import { terminal } from "$lib/terminal.svelte";
  import { requestTerminalSurface } from "$lib/terminal-surface";
  import { unsavedExit } from "$lib/unsaved-exit.svelte";
  import { workspace } from "$lib/workspace.svelte";

  let { children }: { children: Snippet } = $props();

  async function toggleSurface(): Promise<void> {
    await requestTerminalSurface({
      surface: terminal.surface,
      hasUnsaved: workspace.hasUnsaved,
      confirmSave: () => unsavedExit.request("save"),
      saveAll: () => workspace.saveAll(),
      enter: () => terminal.enterTerminals(),
      leave: () => terminal.leaveTerminals(),
    });
  }

  function onWindowKeydown(event: KeyboardEvent): void {
    if (terminal.surface !== "terminals") {
      handleSaveShortcut(event, { save: () => void workspace.save() });
    }

    handleTerminalShortcut(event, {
      hasWorkspace: workspace.root !== null,
      surface: terminal.surface,
      toggle: (dock) => terminal.toggle(dock),
    });
    handleTerminalSurfaceShortcut(event, {
      hasWorkspace: workspace.root !== null,
      toggleSurface: () => void toggleSurface(),
    });
  }
</script>

<svelte:window onkeydowncapture={onWindowKeydown} />

<div class="ide">
  <div class="body">
    {@render children()}
  </div>
  <footer>
    <div class="footer-start">
      <span class="brand">idioteque</span>
      <FooterTransient />
    </div>
    <FooterActions />
  </footer>
</div>

<style>
  .ide {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  footer {
    position: sticky;
    bottom: 0;
    z-index: 10;
    display: flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    height: var(--footer-height);
    padding: 0 0.5rem;
    border-top: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-faint);
  }

  .footer-start {
    display: flex;
    flex: 1;
    min-width: 0;
    align-items: center;
    gap: 0.75rem;
  }

  .brand {
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: lowercase;
  }
</style>
