#!/usr/bin/env node
// Run every test: Rust unit + integration, web unit, then browser tests
// against a fresh build. Usage: node scripts/test-all.mjs [--skip-e2e]
//
// Set CARGO_TARGET_DIR to build somewhere other than server/target (handy
// when a `cargo run` you started elsewhere has the binary locked).

import { spawnSync } from "node:child_process"
import { existsSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
const server = path.join(root, "server")
const web = path.join(root, "web")
const skipE2e = process.argv.includes("--skip-e2e")
const targetDir = process.env.CARGO_TARGET_DIR ?? path.join(server, "target")
const exe = process.platform === "win32" ? "cecilia.exe" : "cecilia"

function run(title, cmd, args, cwd, env = {}) {
  console.log(`\n=== ${title} ===`)
  const r = spawnSync(cmd, args, { cwd, stdio: "inherit", shell: process.platform === "win32", env: { ...process.env, ...env } })
  if (r.status !== 0) {
    console.error(`\n✗ ${title} failed`)
    process.exit(r.status ?? 1)
  }
}

run("Rust format", "cargo", ["fmt", "--check"], server)
run("Rust lint", "cargo", ["clippy", "--all-targets", "--", "-D", "warnings"], server)
run("Rust tests", "cargo", ["test", "--workspace"], server)
run("Web format", "npx", ["prettier", "--check", "**/*.{ts,tsx}"], web)
run("Web type check", "npx", ["tsc", "--noEmit"], web)
run("Web lint", "npx", ["eslint", "."], web)
run("Web unit tests", "npx", ["vitest", "run"], web)
if (!skipE2e) {
  run("Server build", "cargo", ["build", "-p", "organ-server"], server)
  run("Web build", "npm", ["run", "build"], web)
  run("Browser tests", "npx", ["playwright", "test"], web, {
    CECILIA_BIN: path.join(targetDir, "debug", exe),
  })
}
// The community service, when it's cloned alongside.
const community = path.join(root, "..", "cecilia-community")
if (existsSync(path.join(community, "node_modules"))) {
  run("Community type check", "npx", ["tsc", "--noEmit"], community)
  run("Community tests", "npm", ["test"], community)
}
console.log("\n✓ All tests passed")
