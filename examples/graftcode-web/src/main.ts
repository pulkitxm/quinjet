import { GraftConfig, Quinjet } from "quinjet-graft";
import { areas, codes, type Change, type Status } from "./status";

const gateway = import.meta.env.VITE_QUINJET_GATEWAY ?? "ws://localhost:7381/ws";
GraftConfig.host = gateway;

const element = <T extends HTMLElement>(selector: string): T => {
  const found = document.querySelector<T>(selector);
  if (found === null) {
    throw new Error("Missing element " + selector);
  }
  return found;
};

const refreshButton = element<HTMLButtonElement>("#refresh");
const gatewayLabel = element("#gateway");
const callResult = element("#call-result");
const branchLabel = element("#branch");
const changesList = element("#changes");
const errorBox = element("#error");

gatewayLabel.textContent = gateway;

const node = (tag: string, className: string, text?: string): HTMLElement => {
  const created = document.createElement(tag);
  created.className = className;
  if (text !== undefined) {
    created.textContent = text;
  }
  return created;
};

const renderBranch = ({ branch }: Status): void => {
  const head = branch.detached ? "detached HEAD" : (branch.head ?? "no branch");
  const parts = [head];
  if (branch.oid !== null) {
    parts.push(branch.oid.slice(0, 7));
  }
  if (branch.upstream !== null) {
    parts.push(branch.upstream + " +" + branch.ahead + " -" + branch.behind);
  }
  branchLabel.textContent = parts.join("  ·  ");
};

const renderChange = (change: Change): HTMLElement => {
  const row = node("li", "change");
  row.append(node("span", "code status-" + change.status, codes[change.status]));
  const path = node("span", "path");
  if (change.originalPath !== null) {
    path.append(node("span", "original", change.originalPath + " → "));
  }
  path.append(change.path);
  row.append(path, node("span", "kind", change.status));
  return row;
};

const renderChanges = ({ changes }: Status): void => {
  changesList.replaceChildren();
  for (const { area, label } of areas) {
    const inArea = changes.filter((change) => change.area === area);
    if (inArea.length === 0) {
      continue;
    }
    const section = node("section", "area area-" + area);
    const heading = node("h2", "area-title", label);
    heading.append(node("span", "count", String(inArea.length)));
    const list = node("ul", "area-list");
    list.append(...inArea.map(renderChange));
    section.append(heading, list);
    changesList.append(section);
  }
  if (changesList.childElementCount === 0) {
    changesList.append(node("p", "clean", "Working tree clean"));
  }
};

const refresh = async (): Promise<void> => {
  refreshButton.disabled = true;
  errorBox.hidden = true;
  const started = performance.now();
  try {
    const answer = await Quinjet.status("", "");
    const elapsed = Math.round(performance.now() - started);
    const status = JSON.parse(String(answer)) as Status;
    callResult.textContent =
      status.changes.length + " changes in " + elapsed + " ms at " + new Date().toLocaleTimeString();
    renderBranch(status);
    renderChanges(status);
  } catch (error) {
    errorBox.hidden = false;
    errorBox.textContent = error instanceof Error ? error.message : String(error);
    callResult.textContent = "failed";
  } finally {
    refreshButton.disabled = false;
  }
};

refreshButton.addEventListener("click", () => void refresh());
document.addEventListener("keydown", (event) => {
  if (event.key === "r" && !event.metaKey && !event.ctrlKey && !event.altKey) {
    void refresh();
  }
});

void refresh();
