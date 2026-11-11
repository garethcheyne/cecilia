import type { NextConfig } from "next"

// The UI is a static client of the Rust API: `next build` emits plain files
// into `out/`, which organ-server serves from the same port as the API.
const nextConfig: NextConfig = {
  output: "export",
  // Emit /setup/midi/index.html so a plain static file server finds it.
  trailingSlash: true,
}

export default nextConfig
