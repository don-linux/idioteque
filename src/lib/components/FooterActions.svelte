<script lang="ts">
  import Folder from "@lucide/svelte/icons/folder";
  import GitBranch from "@lucide/svelte/icons/git-branch";
  import House from "@lucide/svelte/icons/house";
  import PanelLeft from "@lucide/svelte/icons/panel-left";
  import PanelLeftClose from "@lucide/svelte/icons/panel-left-close";
  import Settings from "@lucide/svelte/icons/settings";
  import SquareTerminal from "@lucide/svelte/icons/square-terminal";
  import { goto } from "$app/navigation";
  import { ROUTES } from "$lib/app-routes";
  import {
    DEFAULT_FOOTER_ACTION_ORDER,
    runFooterAction,
    type FooterActionId,
  } from "$lib/footer-actions";
  import { gitStatus } from "$lib/git";
  import {
    gitFooterStateFromError,
    gitFooterStateFromSnapshot,
    gitFooterTitle,
    type GitFooterState,
  } from "$lib/git-footer";
  import { dockFromAlt } from "$lib/terminal-dock";
  import { terminal } from "$lib/terminal.svelte";
  import { panels } from "$lib/workspace-panels.svelte";
  import { workspace } from "$lib/workspace.svelte";

  const labels: Record<FooterActionId, string> = {
    home: "Inicio",
    folder: "Cambiar",
    settings: "Configuración",
    explorer: "Árbol de archivos",
    terminal: "Terminal",
    git: "Git",
  };

  const titles: Record<FooterActionId, string> = {
    home: "Inicio",
    folder: "Cambiar carpeta",
    settings: "Configuración",
    explorer: "Mostrar u ocultar el árbol de archivos (Ctrl+B)",
    terminal: "Terminal (Ctrl+J) · a la derecha (Ctrl+Alt+J) · pantalla (Ctrl+Shift+J)",
    git: "Git",
  };

  let gitState = $state.raw<GitFooterState>({ kind: "loading" });
  let gitGen = 0;

  async function refreshGit(root: string | null): Promise<void> {
    if (!root) {
      gitState = { kind: "empty" };
      return;
    }
    const gen = ++gitGen;
    try {
      const snap = await gitStatus(root);
      if (gen !== gitGen) return;
      gitState = gitFooterStateFromSnapshot(snap);
    } catch {
      if (gen !== gitGen) return;
      gitState = gitFooterStateFromError();
    }
  }

  $effect(() => {
    const root = workspace.root;
    void refreshGit(root);
  });

  let gitTitle = $derived(gitFooterTitle(gitState));

  function onActionClick(id: FooterActionId, event: MouseEvent): void {
    runFooterAction(id, {
      home: () => {
        void workspace.closeWorkspace().then((left) => {
          if (left) void goto(ROUTES.home);
        });
      },
      folder: () => {
        void workspace.openFolder();
      },
      explorer: () => {
        panels.toggleTree();
      },
      terminal: () => {
        panels.toggleTerminal(dockFromAlt(event.altKey));
      },
    });
  }
</script>

<div class="actions">
  {#each DEFAULT_FOOTER_ACTION_ORDER as id (id)}
    <span class="item">
      {#if id === "settings"}
        <a
          href={ROUTES.settings}
          class="action"
          aria-label={labels.settings}
          title={titles.settings}
          draggable="false"
          onclick={(event) => onActionClick(id, event)}
        >
          <Settings size={16} strokeWidth={1.75} aria-hidden="true" />
        </a>
      {:else}
        <button
          type="button"
          class={[
            "action",
            {
              active:
                (id === "terminal" && (terminal.open || terminal.surface === "terminals")) ||
                (id === "explorer" && panels.treeVisible),
              idle: id === "git",
            },
          ]}
          aria-pressed={id === "terminal"
            ? terminal.open || terminal.surface === "terminals"
            : id === "explorer"
              ? panels.treeVisible
              : undefined}
          aria-label={labels[id]}
          title={id === "git" ? gitTitle : titles[id]}
          onpointerenter={() => {
            if (id === "git") void refreshGit(workspace.root);
          }}
          onclick={(event) => onActionClick(id, event)}
        >
          {#if id === "home"}
            <House size={16} strokeWidth={1.75} aria-hidden="true" />
          {:else if id === "folder"}
            <Folder size={16} strokeWidth={1.75} aria-hidden="true" />
          {:else if id === "git"}
            <GitBranch size={16} strokeWidth={1.75} aria-hidden="true" />
          {:else if id === "explorer"}
            {#if panels.treeVisible}
              <PanelLeftClose size={16} strokeWidth={1.75} aria-hidden="true" />
            {:else}
              <PanelLeft size={16} strokeWidth={1.75} aria-hidden="true" />
            {/if}
          {:else}
            <SquareTerminal size={16} strokeWidth={1.75} aria-hidden="true" />
          {/if}
        </button>
      {/if}
    </span>
  {/each}
</div>

<style>
  .actions {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    margin-left: auto;
    gap: 0.15rem;
    user-select: none;
  }

  .item {
    display: inline-flex;
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
    text-decoration: none;
    cursor: pointer;
  }

  .action.idle {
    cursor: default;
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
