export type ChangeArea = "conflict" | "staged" | "unstaged";

export type ChangeStatus =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "copied"
  | "type-changed"
  | "untracked"
  | "conflicted";

export interface Change {
  path: string;
  originalPath: string | null;
  area: ChangeArea;
  status: ChangeStatus;
}

export interface Branch {
  head: string | null;
  oid: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  detached: boolean;
}

export interface Status {
  branch: Branch;
  changes: Change[];
}

export const areas: { area: ChangeArea; label: string }[] = [
  { area: "conflict", label: "Merge Changes" },
  { area: "staged", label: "Staged Changes" },
  { area: "unstaged", label: "Changes" },
];

export const codes: Record<ChangeStatus, string> = {
  added: "A",
  modified: "M",
  deleted: "D",
  renamed: "R",
  copied: "C",
  "type-changed": "T",
  untracked: "U",
  conflicted: "!",
};
