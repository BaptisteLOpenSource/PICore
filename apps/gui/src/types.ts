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

export type WorktreeHealth = {
  path: string;
  branch: string | null;
  dirty: number | null;
  ahead: number | null;
  behind: number | null;
  upstream: string | null;
  error: string | null;
};

export type ProjectHealth = {
  worktrees: WorktreeHealth[];
  remote: Check | null;
};

// Rust `view::ProjectView` + health. `id` = Project path, `worktrees` = count incl. main.
export type Listed = {
  id: string;
  name: string;
  product: string | null;
  branch: string | null;
  repo: string | null;
  techs: string[];
  worktrees: number;
  health: ProjectHealth;
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
  roots: string[];
  warnings: string[];
};

export type Detail = {
  context: ProjectContext;
  health: ProjectHealth;
};
