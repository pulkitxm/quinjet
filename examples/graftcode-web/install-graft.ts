import { execFileSync } from "node:child_process";

const vision = process.env.QUINJET_GATEWAY_HTTP ?? "http://localhost:7381";
const alias = "quinjet-graft";

const response = await fetch(new URL("/npm", vision)).catch((error: unknown) => {
  throw new Error(
    "No Graftcode gateway answered at " + vision + ". Start one with: quinjet graft serve --publish",
    { cause: error },
  );
});
const command = (await response.text()).trim();

const registry = command.match(/--registry\s+(\S+)/)?.[1];
const pkg = command.match(/(@graft\/npm-quinjet-graft\S*)/)?.[1];
if (registry === undefined || pkg === undefined) {
  throw new Error(
    "The gateway at " + vision + " has no published npm Graft. Restart it with: quinjet graft serve --publish",
  );
}

process.stdout.write("Installing " + pkg + " as " + alias + "\n");
execFileSync(
  "npm",
  ["install", "--no-save", "--registry", registry, alias + "@npm:" + pkg],
  { stdio: "inherit", shell: process.platform === "win32" },
);
