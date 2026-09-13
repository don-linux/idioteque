import { invoke } from "@tauri-apps/api/core";

export type GitFileKind =
  | "ordinary"
  | "renamed"
  | "unmerged"
  | "untracked"
  | "ignored";

export interface GitFile {
  path: string;
  originalPath?: string;
  staged: string;
  unstaged: string;
  kind: GitFileKind;
}

export interface GitProbe {
  available: boolean;
  path?: string;
  version?: string;
}

export interface GitRepository {
  toplevel: string;
  gitDir: string;
  branch?: string;
  oid?: string;
  detached: boolean;
  initial: boolean;
  upstream?: string;
  ahead: number;
  behind: number;
  dirty: boolean;
  files: GitFile[];
}

export interface GitSnapshot {
  probe: GitProbe;
  repository?: GitRepository;
}

export interface GitRef {
  name: string;
  hash?: string;
  current: boolean;
}

export interface GitRefRepository {
  current?: string;
  oid?: string;
  detached: boolean;
  initial: boolean;
  branches: GitRef[];
}

export interface GitRefsSnapshot {
  probe: GitProbe;
  repository?: GitRefRepository;
}

export interface GitCommit {
  hash: string;
  short: string;
  parents: string[];
  subject: string;
  refs: string[];
}

export interface GitComparison {
  name: string;
  mergeBase?: string;
  ahead: number;
  behind: number;
}

export interface GitGraphRepository {
  current?: string;
  oid?: string;
  detached: boolean;
  initial: boolean;
  commits: GitCommit[];
  comparisons: GitComparison[];
}

export interface GitGraphSnapshot {
  probe: GitProbe;
  repository?: GitGraphRepository;
}

export function probeGit(): Promise<GitProbe> {
  return invoke("git_probe");
}

export function gitStatus(root: string): Promise<GitSnapshot> {
  return invoke("git_status", { root });
}

export function gitRefs(root: string): Promise<GitRefsSnapshot> {
  return invoke("git_refs", { root });
}

export function gitGraphLog(root: string, selected: string[]): Promise<GitGraphSnapshot> {
  return invoke("git_graph", { root, selected });
}

export function isUntracked(file: GitFile): boolean {
  return file.kind === "untracked";
}

export function hasStagedChange(file: GitFile): boolean {
  return file.kind !== "untracked" && file.staged !== ".";
}

export function hasUnstagedChange(file: GitFile): boolean {
  return file.kind === "untracked" || file.unstaged !== ".";
}

export function isConflict(file: GitFile): boolean {
  return file.kind === "unmerged";
}
