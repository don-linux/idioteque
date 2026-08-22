<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import ToastHost from "$lib/components/ToastHost.svelte";
  import UnsavedExitModal from "$lib/components/UnsavedExitModal.svelte";
  import { appConfig } from "$lib/app-config.svelte";
  import { settingsEditor } from "$lib/settings-editor.svelte";
  import { unsavedExit } from "$lib/unsaved-exit.svelte";
  import { applyTheme } from "$lib/ui-theme";
  import { workspace } from "$lib/workspace.svelte";

  let { children }: { children: Snippet } = $props();

  $effect(() => {
    applyTheme(document.documentElement, settingsEditor.uiTheme);
  });

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
    --bg: #1c1e22;
    --surface: #24272d;
    --surface-hover: #2c3038;
    --border: #3a404a;
    --text: #d2d5db;
    --text-muted: #8f96a1;
    --text-faint: #6a7080;
    --accent: #7b9ee8;
    --accent-soft: #7b9ee822;
    --danger: #e08b99;
    --shadow: #00000055;
    --syntax-heading: #8eb0ee;
    --syntax-comment: #6a7080;
    --syntax-link: #7b9ee8;
    --syntax-code: #c4a882;
    --syntax-keyword: #7b9ee8;
    --syntax-string: #c4a882;
    --syntax-number: #e08b99;
    --syntax-function: #8eb0ee;
    --syntax-type: #c4a882;
    --syntax-variable: #d2d5db;
    --syntax-operator: #8f96a1;
    --syntax-tag: #e08b99;
    --syntax-invalid: #e08b99;
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
		display: flex;
		flex: 1;
		flex-direction: column;
		width: 100%;
		height: 100%;
		min-height: 0;
		overflow: hidden;
	}
</style>
