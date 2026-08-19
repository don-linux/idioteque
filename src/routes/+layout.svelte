<script lang="ts">
  import { onMount } from "svelte";
  import { appConfig } from "$lib/app-config.svelte";

  let { children } = $props();

  onMount(() => {
    void appConfig.load();
  });
</script>

<div class="shell">
  <div class="page">
    {@render children()}
  </div>
  <footer>
    <span>idioteque</span>
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
    overflow: auto;
  }

  footer {
    position: sticky;
    bottom: 0;
    z-index: 10;
    display: flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    height: var(--footer-height);
    border-top: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-faint);
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: lowercase;
  }
</style>
