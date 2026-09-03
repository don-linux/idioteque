<script lang="ts">
  import type { Snippet } from "svelte";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import FooterActions from "$lib/components/FooterActions.svelte";
  import FooterTransient from "$lib/components/FooterTransient.svelte";
  import { FOLDER_VISIBILITY_LABEL } from "$lib/folder-visibility";
  import { handleTreeToggleShortcut, isTerminalTarget } from "$lib/panel-shortcuts";
  import { handleSaveShortcut } from "$lib/save-shortcut";
  import { handleTerminalShortcut, handleTerminalSurfaceShortcut } from "$lib/terminal-dock";
  import { terminal } from "$lib/terminal.svelte";
  import { requestTerminalSurface } from "$lib/terminal-surface";
  import { unsavedExit } from "$lib/unsaved-exit.svelte";
  import { panels } from "$lib/workspace-panels.svelte";
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
      toggle: (dock) => panels.toggleTerminal(dock),
    });
    handleTerminalSurfaceShortcut(event, {
      hasWorkspace: workspace.root !== null,
      toggleSurface: () => void toggleSurface(),
    });
    handleTreeToggleShortcut(event, {
      hasWorkspace: workspace.root !== null,
      insideTerminal: isTerminalTarget(event.target),
      toggleTree: () => panels.toggleTree(),
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
      <div class="brand-group">
        <span class="brand">idioteque</span>
        {#if workspace.canEditVisibility}
          <button
            type="button"
            class="action"
            aria-label={FOLDER_VISIBILITY_LABEL}
            title={FOLDER_VISIBILITY_LABEL}
            onclick={() => void workspace.editVisibility()}
          >
            <FolderPlus size={16} strokeWidth={1.75} aria-hidden="true" />
          </button>
        {/if}
      </div>
      <FooterTransient />
    </div>
    <FooterActions />
  </footer>
</div>

<style>
	.ide {
		display: flex;
		flex: 1 1 0;
		flex-direction: column;
		width: 100%;
		height: 100%;
		min-height: 0;
	}

	.body {
		position: relative;
		display: flex;
		flex: 1 1 0;
		flex-direction: column;
		width: 100%;
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

  .brand-group {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 0.2rem;
  }

  .brand {
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: lowercase;
  }

  .action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.65rem;
    height: 1.65rem;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .action:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
</style>
