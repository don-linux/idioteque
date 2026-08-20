<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import ToastHost from "$lib/components/ToastHost.svelte";
  import UnsavedExitModal from "$lib/components/UnsavedExitModal.svelte";
  import { appConfig } from "$lib/app-config.svelte";
  import { unsavedExit } from "$lib/unsaved-exit.svelte";
  import { workspace } from "$lib/workspace.svelte";

  let { children }: { children: Snippet } = $props();

  onMount(() => {
    void appConfig.load();

    let unlisten: (() => void) | undefined;
    try {
      void getCurrentWindow()
        .onCloseRequested(async (event) => {
          if (!workspace.hasUnsaved) return;
          event.preventDefault();
          const confirmed = await unsavedExit.request();
          if (!confirmed) return;
          workspace.discardUnsaved();
          await getCurrentWindow().destroy();
        })
        .then((fn) => {
          unlisten = fn;
        });
    } catch {
      // Browser preview without the Tauri runtime.
    }

    return () => unlisten?.();
  });
</script>

<div class="shell">
  <div class="page">
    {@render children()}
  </div>
</div>
<UnsavedExitModal />
<ToastHost />

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
</style>
