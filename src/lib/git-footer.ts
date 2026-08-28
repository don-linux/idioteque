import type { GitSnapshot } from "$lib/git";

export type GitFooterState =
  | { kind: "loading" }
  | { kind: "unavailable" }
  | { kind: "empty" }
  | { kind: "error" }
  | { kind: "repo"; name: string; branch?: string; detached: boolean };

export function repoDisplayName(toplevel: string): string {
  const trimmed = toplevel.trim().replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/).filter((part) => part.length > 0 && part !== ".");
  return parts.at(-1) ?? "";
}

export function gitFooterStateFromSnapshot(snapshot: GitSnapshot): GitFooterState {
  if (!snapshot.probe.available) {
    return { kind: "unavailable" };
  }

  const repository = snapshot.repository;
  if (!repository) {
    return { kind: "empty" };
  }

  const name = repoDisplayName(repository.toplevel);
  if (!name) {
    return { kind: "empty" };
  }

  if (repository.detached || !repository.branch?.trim()) {
    return { kind: "repo", name, detached: true };
  }

  return { kind: "repo", name, branch: repository.branch.trim(), detached: false };
}

export function gitFooterTitle(state: GitFooterState): string {
  switch (state.kind) {
    case "loading":
      return "Git";
    case "unavailable":
      return "Git no está disponible";
    case "empty":
      return "Sin repositorio Git";
    case "error":
      return "Git no responde";
    case "repo":
      if (state.detached) return `${state.name} · HEAD separado`;
      if (state.branch) return `${state.name} · ${state.branch}`;
      return state.name;
  }
}

export function gitFooterStateFromError(): GitFooterState {
  return { kind: "error" };
}
