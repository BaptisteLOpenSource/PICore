export type Status = "ok" | "ko" | "unknown";

export type Check = {
  name: string;
  status: Status;
  message: string;
  hint: string | null;
};

export type Worktree = {
  path: string;
  branch: string | null;
  head: string | null;
  is_main: boolean;
  detached: boolean;
  locked: boolean;
  last_commit_at: number | null;
};

export type ProjectHealth = {
  dirty: number;
  ahead: number;
  behind: number;
  upstream: string | null;
  remote: Check | null;
};

export type Listed = {
  id: string;
  name: string;
  work_tree: string | null;
  branch: string | null;
  techs: string[];
  worktrees: Worktree[];
  stamp: number;
  product: string | null;
  health: ProjectHealth | null;
};

export type Heading = { level: number; text: string };

export type FileOutline = {
  file: string;
  intro: string;
  intro_truncated: boolean;
  headings: Heading[];
  headings_truncated: boolean;
};

export type ProjectContext = {
  name: string;
  product: string | null;
  branch: string | null;
  techs: string[];
  worktrees: Worktree[];
  files: FileOutline[];
};

export type Overview = {
  projects: Listed[];
  products: Record<string, string>;
  warnings: string[];
};

export type Detail = {
  context: ProjectContext;
  health: ProjectHealth;
};
