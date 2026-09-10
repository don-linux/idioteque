export type SidebarView = "tree" | "git";

export interface SidebarState {
  visible: boolean;
  view: SidebarView;
}

/** Ctrl+B / PanelLeft: always land on the file tree. Hide only when the tree is already showing. */
export function applyTreeToggle(state: SidebarState): SidebarState {
  if (!state.visible || state.view === "git") {
    return { visible: true, view: "tree" };
  }

  return { visible: false, view: "tree" };
}

/** Ctrl+G / footer Git: always land on the graph. Hide only when the graph is already showing. */
export function applyGitToggle(state: SidebarState): SidebarState {
  if (!state.visible || state.view === "tree") {
    return { visible: true, view: "git" };
  }

  return { visible: false, view: "git" };
}

export function applyShowTree(): SidebarState {
  return { visible: true, view: "tree" };
}

export function applyShowGit(): SidebarState {
  return { visible: true, view: "git" };
}

export function isGitSidebar(state: SidebarState): boolean {
  return state.visible && state.view === "git";
}

export function isTreeSidebar(state: SidebarState): boolean {
  return state.visible && state.view === "tree";
}

/** True when opening git would leave the file tree (selection should reset to the current branch). */
export function gitOpensFromTree(state: SidebarState): boolean {
  return state.view === "tree";
}
