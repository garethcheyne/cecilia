import fs from "node:fs"
import os from "node:os"
import path from "node:path"

import { fileURLToPath } from "node:url"

import { defineConfig, devices } from "@playwright/test"

const here = path.dirname(fileURLToPath(import.meta.url))

// Browser tests run against the real organ-server serving the static build
// (`npm run build` first). Each run gets an empty data folder and no sound.
const PORT = Number(process.env.E2E_PORT ?? 7090)
const exe = process.platform === "win32" ? "cecilia.exe" : "cecilia"
const bin = process.env.CECILIA_BIN ?? path.resolve(here, "../server/target/debug", exe)
const dataDir = fs.mkdtempSync(path.join(os.tmpdir(), "cecilia-e2e-"))

export default defineConfig({
  testDir: "e2e",
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: "retain-on-failure",
    // Use the installed Edge or Chrome rather than downloading a browser.
    channel: process.env.E2E_CHANNEL ?? (process.platform === "win32" ? "msedge" : "chrome"),
  },
  projects: [
    { name: "desktop", use: { viewport: { width: 1280, height: 900 } } },
    { name: "phone", use: { ...devices["Pixel 7"], channel: undefined } },
  ],
  webServer: {
    command: `"${bin}"`,
    url: `http://127.0.0.1:${PORT}/api/v1/health`,
    reuseExistingServer: false,
    env: {
      CECILIA_ADDR: `127.0.0.1:${PORT}`,
      CECILIA_DATA_DIR: dataDir,
      CECILIA_WEB_DIR: path.resolve(here, "out"),
      CECILIA_AUDIO: "0",
    },
  },
})
