#!/usr/bin/env node
/**
 * Bump [workspace.package].version in Cargo.toml (+ matching Cargo.lock
 * entries for workspace members) from a Conventional Commit PR title.
 *
 * Intended for post-merge on `main` (model B). Optional base-version is
 * kept for local/manual use.
 *
 * Usage:
 *   node .github/scripts/update-version.mjs "<pr-title>" [base-version]
 *   node .github/scripts/update-version.mjs "<pr-title>" --should-tag
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "../..");
const CARGO_TOML = path.join(ROOT, "Cargo.toml");
const CARGO_LOCK = path.join(ROOT, "Cargo.lock");

const WORKSPACE_PACKAGES = new Set([
  "astrowar",
  "astrowar-server",
  "net",
  "protocol",
]);

const PREFIX_LEVEL = {
  "fix:": "patch",
  "fix(": "patch",
  "refact:": "patch",
  "refact(": "patch",
  "refactor:": "patch",
  "refactor(": "patch",
  "style:": "patch",
  "style(": "patch",
  "perf:": "patch",
  "perf(": "patch",
  "feat:": "minor",
  "feat(": "minor",
  "breaking:": "major",
  "breaking(": "major",
};

function bumpLevel(title = "") {
  if (/^[a-z]+(\([^)]*\))?!:/.test(title)) {
    return "major";
  }
  for (const [prefix, level] of Object.entries(PREFIX_LEVEL)) {
    if (title.startsWith(prefix)) {
      return level;
    }
  }
  return null;
}

function parseVersion(version) {
  const [major, minor, patch] = version.split(".").map((p) => Number.parseInt(p, 10) || 0);
  return [major, minor, patch];
}

function formatVersion([major, minor, patch]) {
  return `${major}.${minor}.${patch}`;
}

function isGreater(a, b) {
  const left = parseVersion(a);
  const right = parseVersion(b);
  for (let i = 0; i < 3; i++) {
    if (left[i] > right[i]) return true;
    if (left[i] < right[i]) return false;
  }
  return false;
}

function bump(version, level) {
  const [major, minor, patch] = parseVersion(version);
  if (level === "major") return formatVersion([major + 1, 0, 0]);
  if (level === "minor") return formatVersion([major, minor + 1, 0]);
  return formatVersion([major, minor, patch + 1]);
}

function readWorkspaceVersion(toml) {
  const block = toml.match(/\[workspace\.package\][\s\S]*?(?=\n\[|$)/);
  if (!block) {
    throw new Error("Missing [workspace.package] in Cargo.toml");
  }
  const match = block[0].match(/version\s*=\s*"([^"]+)"/);
  if (!match) {
    throw new Error("Missing version in [workspace.package]");
  }
  return match[1];
}

function writeWorkspaceVersion(toml, next) {
  let inWorkspacePackage = false;
  const lines = toml.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (/^\[workspace\.package\]/.test(line)) {
      inWorkspacePackage = true;
      continue;
    }
    if (inWorkspacePackage && /^\[/.test(line)) {
      inWorkspacePackage = false;
    }
    if (inWorkspacePackage && /^\s*version\s*=/.test(line)) {
      lines[i] = line.replace(/version\s*=\s*"[^"]+"/, `version = "${next}"`);
      return lines.join("\n");
    }
  }
  throw new Error("Could not rewrite workspace.package version");
}

function writeLockVersions(lock, next) {
  // Rewrite version only for known workspace packages in [[package]] tables.
  return lock.replace(
    /\[\[package\]\]\nname = "([^"]+)"\nversion = "[^"]+"/g,
    (full, name) => {
      if (!WORKSPACE_PACKAGES.has(name)) {
        return full;
      }
      return `[[package]]\nname = "${name}"\nversion = "${next}"`;
    },
  );
}

function main() {
  const title = process.argv[2] ?? "";
  const level = bumpLevel(title);

  if (process.argv.includes("--should-tag")) {
    process.stdout.write(level ? "yes" : "no");
    return;
  }

  if (!level) {
    console.log("No version update needed");
    return;
  }

  const toml = fs.readFileSync(CARGO_TOML, "utf8");
  const current = readWorkspaceVersion(toml);
  const baseArg = process.argv[3];
  const baseVersion =
    baseArg && !baseArg.startsWith("--") ? baseArg : undefined;

  // Prefer bumping from main's version so a rebase onto a newer main
  // still produces the correct next semver (e.g. fix on 0.2.0 → 0.2.1).
  const start = baseVersion ?? current;

  if (baseVersion && isGreater(current, start)) {
    const already = bump(start, level);
    if (current === already || isGreater(current, already)) {
      console.log(
        `Already ahead of base (${current} vs ${start}); skipping`,
      );
      return;
    }
  }

  const next = bump(start, level);
  if (current === next) {
    console.log(`Already at ${next}; skipping`);
    return;
  }

  fs.writeFileSync(CARGO_TOML, writeWorkspaceVersion(toml, next));

  if (fs.existsSync(CARGO_LOCK)) {
    const lock = fs.readFileSync(CARGO_LOCK, "utf8");
    fs.writeFileSync(CARGO_LOCK, writeLockVersions(lock, next));
  }

  console.info(`Version updated to "${next}" (${level})`);
}

main();
