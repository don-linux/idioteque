<script lang="ts">
  import type { Snippet } from "svelte";
  import FooterActions from "$lib/components/FooterActions.svelte";
  import { handleTerminalShortcut } from "$lib/terminal-dock";
  import { terminal } from "$lib/terminal.svelte";
  import { workspace } from "$lib/workspace.svelte";

  let { children }: { children: Snippet } = $props();

  function onWindowKeydown(event: KeyboardEvent): void {
    handleTerminalShortcut(event, {
      hasWorkspace: workspace.root !== null,
      toggle: (dock) => terminal.toggle(dock),
    });
  }
</script>

<svelte:window onkeydowncapture={onWindowKeydown} />

<div class="ide">
  <div class="body">
    {@render children()}
  </div>
  <footer>
    <span class="brand">idioteque</span>
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
    flex: 1;
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

  .brand {
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: lowercase;
  }
</style>
