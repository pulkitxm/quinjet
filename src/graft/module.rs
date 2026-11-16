use std::fmt::Write as _;

use anyhow::Result;

use super::receiver::{CLASS_NAME, Kind, Method, REPO_PARAM, Receiver, TOKEN_PARAM};

const RUNTIME: &str = r#""use strict";

const { execFile } = require("node:child_process");
const { closeSync, openSync, utimesSync } = require("node:fs");

const OUTPUT_LIMIT = 64 * 1024 * 1024;

function present(value) {
  return value !== undefined && value !== null && value !== "" && value !== false;
}

function enabled(value) {
  return value === true || value === 1 || value === "1" || value === "true" || value === "yes";
}

function items(value, list) {
  if (!list) {
    return [String(value)];
  }
  const values = Array.isArray(value) ? value : String(value).split(/\s+/);
  return values.map(String).filter((item) => item.length > 0);
}

function argv(spec, values) {
  const options = [];
  const positionals = [];
  spec.params.forEach((param, index) => {
    const value = values[index];
    if (!present(value)) {
      return;
    }
    if (param.kind === "switch") {
      if (enabled(value)) {
        options.push(param.flag);
      }
      return;
    }
    for (const item of items(value, param.list)) {
      if (param.kind === "option") {
        options.push(param.flag, item);
      } else {
        positionals.push(item);
      }
    }
  });
  const tail = positionals.length > 0 ? ["--", ...positionals] : [];
  return [...spec.path, ...options, "--json", ...tail];
}

function touch() {
  const path = process.env.QUINJET_GRAFT_ACTIVITY;
  if (!path) {
    return;
  }
  try {
    const now = new Date();
    utimesSync(path, now, now);
  } catch {
    try {
      closeSync(openSync(path, "a"));
    } catch {
      return;
    }
  }
}

function run(binary, args) {
  return new Promise((resolve) => {
    execFile(
      binary,
      args,
      {
        encoding: "utf8",
        maxBuffer: OUTPUT_LIMIT,
        windowsHide: true,
        env: { ...process.env, NO_COLOR: "1" },
      },
      (error, stdout, stderr) => resolve({ error, stdout, stderr }),
    );
  });
}

async function attempt(name, values, repoPath, accessToken) {
  const spec = SPECS[name];
  const expected = process.env.QUINJET_GRAFT_TOKEN || "";
  if (expected && accessToken !== expected) {
    return { failure: "this Quinjet gateway is shared; connect through `quinjet mcp`" };
  }
  const repository = present(repoPath) ? String(repoPath) : process.env.QUINJET_GRAFT_REPO || "";
  if (!repository) {
    return { failure: "pass repoPath, the repository to run against" };
  }
  const binary = process.env.QUINJET_GRAFT_BIN || "quinjet";
  const result = await run(binary, ["-C", repository, ...argv(spec, values)]);
  if (!result.error) {
    return { stdout: result.stdout };
  }
  const detail = (result.stderr || "").trim() || result.error.message;
  const status = typeof result.error.code === "number" ? result.error.code : result.error.code || "";
  return { failure: `quinjet ${spec.path.join(" ")} exited ${status}: ${detail}` };
}

async function invoke(name, values, repoPath, accessToken) {
  touch();
  const outcome = await attempt(name, values, repoPath, accessToken);
  if (outcome.failure !== undefined) {
    throw new Error(outcome.failure);
  }
  return outcome.stdout;
}
"#;

pub(crate) fn package_json(version: &str) -> Result<String> {
    let document = serde_json::json!({
        "name": "quinjet-graft",
        "version": version,
        "private": true,
        "description": "Quinjet's Git and GitHub commands as Graftcode methods",
        "main": "index.js",
    });
    Ok(serde_json::to_string_pretty(&document)? + "\n")
}

pub(crate) fn index_js(receiver: &Receiver) -> Result<String> {
    let specs: serde_json::Map<String, serde_json::Value> = receiver
        .methods
        .iter()
        .map(|method| Ok((method.name.clone(), serde_json::to_value(method)?)))
        .collect::<Result<_>>()?;
    let mut source = String::from(RUNTIME);
    source.push_str("\nconst SPECS = ");
    source.push_str(&serde_json::to_string_pretty(&specs)?);
    source.push_str(";\n\nclass ");
    source.push_str(CLASS_NAME);
    source.push_str(" {\n");
    for method in &receiver.methods {
        write_method(&mut source, method)?;
    }
    source.push_str("}\n\nmodule.exports = { ");
    source.push_str(CLASS_NAME);
    source.push_str(" };\n");
    Ok(source)
}

fn write_method(source: &mut String, method: &Method) -> Result<()> {
    source.push_str("  /**\n   * ");
    source.push_str(&doc_text(&method.about));
    source.push('\n');
    for param in &method.params {
        writeln!(
            source,
            "   * @param {{{}}} {} {}",
            if param.kind == Kind::Switch {
                "boolean"
            } else {
                "string"
            },
            param.name,
            doc_text(&param.description())
        )?;
    }
    writeln!(
        source,
        "   * @param {{string}} {REPO_PARAM} Repository to run against. Optional, pass \"\" for the gateway's own repository"
    )?;
    writeln!(
        source,
        "   * @param {{string}} {TOKEN_PARAM} Access token of a shared Quinjet gateway. Optional, pass \"\" when the gateway has none"
    )?;
    source.push_str("   * @returns {string} Quinjet's JSON answer\n   */\n  static async ");
    let names: Vec<&str> = method
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    let values = names.join(", ");
    let mut signature = names.clone();
    signature.extend([REPO_PARAM, TOKEN_PARAM]);
    writeln!(
        source,
        "{}({}) {{\n    return invoke(\"{}\", [{values}], {REPO_PARAM}, {TOKEN_PARAM});\n  }}\n",
        method.name,
        signature.join(", "),
        method.name
    )?;
    Ok(())
}

fn doc_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("*/", "* /")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documentation_text_cannot_close_the_block() {
        assert_eq!(doc_text("a */ b\n  c"), "a * / b c");
    }

    #[test]
    fn every_method_is_async_and_takes_the_repository_and_token_last() {
        let receiver = crate::cli::graft_receiver(false);
        let source = index_js(&receiver).unwrap();
        assert!(source.contains(
            "static async status(repoPath, accessToken) {\n    return invoke(\"status\", [], repoPath, accessToken);"
        ));
        assert!(source.contains(
            "static async prChecks(number, commit, repo, refresh, repoPath, accessToken)"
        ));
        assert!(!source.contains("spawnSync"));
    }
}
