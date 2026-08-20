<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import SquareTerminal from "@lucide/svelte/icons/square-terminal";
  import { appConfig } from "$lib/app-config.svelte";
  import { dockFromAlt, handleTerminalShortcut } from "$lib/terminal-dock";
  import { terminal } from "$lib/terminal.svelte";
  import { workspace } from "$lib/workspace.svelte";

  let { children }: { children: Snippet } = $props();

  onMount(() => {
    void appConfig.load();
  });

  function onWindowKeydown(event: KeyboardEvent): void {
    handleTerminalShortcut(event, {
      hasWorkspace: workspace.root !== null,
      toggle: (dock) => terminal.toggle(dock),
    });
  }

  function onTerminalIconClick(event: MouseEvent): void {
    terminal.toggle(dockFromAlt(event.altKey));
  }
</script>

<svelte:window onkeydowncapture={onWindowKeydown} />

<div class="shell">
  <div class="page">
    {@render children()}
  </div>
  <footer>
    <span class="brand">idioteque</span>
    {#if workspace.root !== null}
      <div class="actions">
        <button
          type="button"
          class={["action", { active: terminal.open }]}
          aria-pressed={terminal.open}
          aria-label="Terminal"
          title="Terminal (Ctrl+J) · a la derecha (Ctrl+Alt+J)"
          onclick={onTerminalIconClick}
        >
          <SquareTerminal size={16} strokeWidth={1.75} aria-hidden="true" />
        </button>
      </div>
    {/if}
  </footer>
</div>

<style>
  :global(:root) {
    --bg: #14161a;
    --surface: #191c21;
    --surface-hover: #22262d;
    --border: #2a2f37;
    --text: #e4e6ea;
    --text-muted: #9aa1ad;
    --text-faint: #666d79;
    --accent: #7aa2f7;
    --accent-soft: #7aa2f722;
    --danger: #f7768e;
    --font-ui: Inter, system-ui, -apple-system, sans-serif;
    --font-mono: "JetBrains Mono", "SF Mono", ui-monospace, monospace;
    --footer-height: 2.75rem;
    --term-size: 280px;

    color-scheme: dark;
  }

  :global(html),
  :global(body) {
    height: 100%;
    margin: 0;
  }

  :global(body) {
    background: var(--bg);
    color: var(--text);
    font-family: var(--font-ui);
    -webkit-font-smoothing: antialiased;
  }

  .shell {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .page {
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

  .actions {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    margin-left: auto;
    gap: 0.15rem;
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

  .action.active {
    background: var(--accent-soft);
    color: var(--accent);
  }
</style>
