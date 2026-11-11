# Registra (working name)

A modern, cross-platform organ MIDI recorder and player, written from scratch.
Its feature set is modelled on the *publicly documented behaviour* of OrganAssist
(see its user manual), but **no OrganAssist code, binaries or file formats are used**.

```
┌────────────────────────┐   HTTP + WebSocket    ┌──────────────────────────┐
│ web/  (Next.js+shadcn) │ ───────────────────▶ │ server/ (Rust, headless) │ ── MIDI ──▶ organ / Hauptwerk
│ static client of API   │ ◀─── live events ─── │ organ-core + organ-server│ ◀─ MIDI ──
└────────────────────────┘                       └──────────────────────────┘
```

* **`server/crates/organ-core`** – MIDI engine library. One OS thread owns every
  MIDI connection; callers use the cloneable `MidiHub` handle and subscribe to
  `EngineEvent`s. No UI or web dependencies, so it can also run embedded (e.g. a
  Raspberry Pi inside a console).
* **`server/crates/organ-server`** – axum HTTP/WebSocket API around the core,
  with an OpenAPI document. Also serves the built web UI, so production is a
  single binary + a folder of static files.
* **`web/`** – Next.js 16 static export with shadcn/ui. Talks to the server only
  through the API; TypeScript types are generated from the server's OpenAPI.

## Running (development)

```sh
# terminal 1 – API on http://127.0.0.1:7070
cd server
cargo run

# terminal 2 – UI on http://localhost:3000 (hot reload)
cd web
npm run dev
```

## Running (production-style)

```sh
cd web && npm run build          # -> web/out
cd ../server && cargo run --release
# open http://127.0.0.1:7070 – API and UI on one port
```

## Changing the API

The Rust types are the source of truth. After changing them:

```sh
cd web && npm run gen:api        # regenerates lib/api/openapi.json + schema.d.ts
```

## Configuration (environment variables)

| Variable            | Default                                         | Purpose                                   |
| ------------------- | ----------------------------------------------- | ----------------------------------------- |
| `REGISTRA_ADDR`     | `127.0.0.1:7070`                                | Listen address. Use `0.0.0.0:7070` for LAN access (no auth yet!) |
| `REGISTRA_DATA_DIR` | OS data dir + `/Registra`                       | `settings.json`, later the music library  |
| `REGISTRA_WEB_DIR`  | `./web` beside the binary, else `../web/out`    | Static UI to serve                        |
| `REGISTRA_CORS`     | `http://localhost:3000,http://127.0.0.1:3000`   | Browser origins allowed to call the API   |
| `REGISTRA_MCP_TOKEN`| unset                                           | If set, `/mcp` requires `Authorization: Bearer <token>`. Set it before exposing the server on a network |
| `RUST_LOG`          | `info`                                          | Log filter                                |

## API (v1)

| Method | Path                        | Description                                   |
| ------ | --------------------------- | --------------------------------------------- |
| GET    | `/api/v1/health`            | Name + version                                |
| GET    | `/api/v1/openapi.json`      | OpenAPI 3.1 document                          |
| GET    | `/api/v1/events`            | WebSocket: `status` snapshots + `midi` traffic |
| GET    | `/api/v1/midi/ports`        | Available MIDI inputs/outputs                 |
| GET/PUT| `/api/v1/midi/config`       | Saved port selection (PUT reconnects)         |
| GET    | `/api/v1/midi/status`       | Open ports, missing ports, test recorder      |
| POST   | `/api/v1/midi/panic`        | All Notes Off / All Sound Off, all channels   |
| POST   | `/api/v1/midi/test/record`  | Start raw test recording                      |
| POST   | `/api/v1/midi/test/stop`    | Stop recording/playback                       |
| POST   | `/api/v1/midi/test/play`    | Replay the raw recording to the output        |
| GET/PUT| `/api/v1/assistant`         | The assistant's name (MCP / voice wake word)  |
| POST   | `/mcp`                      | MCP server (Streamable HTTP), see below       |

## Targets

`organ-core` uses `midir` (WinMM on Windows, CoreMIDI on macOS, ALSA on Linux),
so the server builds for x64 and ARM64 on Windows, macOS and Linux:

```sh
rustup target add aarch64-pc-windows-msvc   # needs the MSVC ARM64 build tools
cargo build --release --target aarch64-pc-windows-msvc
```

## Roadmap

The full feature spec is in [docs/SPEC.md](docs/SPEC.md).

1. ✅ MIDI devices, live monitor, raw test record/playback
2. ✅ Organ definitions (JSON) + generic driver, built-in **simulator** (virtual MIDI port, server-side synth, playable console UI), Makin/Johannus toggle stops and general cancel
3. Auto-recorder → neutral tracks in the library; bundled sample church music
4. Playback engine: live MIDI generation, virtual organist, tempo, verses, fade
5. Library, playlists, MIDI/MusicXML import & export
6. Track editor (piano roll) and score view (staff notation)
7. Organ designer + MIDI learn, combinations, more manufacturer drivers
8. Settings, i18n, backups, auth, Raspberry Pi packaging
9. Scanned music (OMR)

## Simulator

`organ-server` exposes a virtual MIDI device called **Registra Simulator**. Choose it as
Organ in/out on MIDI Setup and everything (recording, playback, monitoring) works
with no instrument attached. The simulator's sound is rendered by the server
itself (WASAPI / CoreAudio / ALSA), so a Raspberry Pi can drive speakers
directly. Set `REGISTRA_AUDIO=0` to run silent.

Built-in organs: **Registra Demo Organ** (on/off stops) and **Makin 2-manual
(template)**. The Makin one behaves like a Makin/Johannus console: each stop is a
toggling Program Change on channel 12, and General cancel sends
`F0 00 4A 4F 48 41 53 7F F7`. Its channels come from Makin's manual. Its stop
list and stop numbers are placeholders until they're learned from a real organ.

## Assistant (MCP server)

`organ-server` is also a [Model Context Protocol](https://modelcontextprotocol.io)
server at `/mcp` (Streamable HTTP, JSON responses, stateless), so an AI assistant can
run the organ. The assistant is called **Cecilia** by default (renamable on Setup → Assistant);
the same tools will back offline voice control with "Cecilia" as the wake word.

```sh
claude mcp add --transport http registra http://127.0.0.1:7070/mcp
```

| Tool | Does |
| ---- | ---- |
| `organ_status` | Organ, ports, drawn stops, swell pedals, recorder state |
| `list_stops` | Stops by division, marking drawn ones |
| `draw_stops` / `push_in_stops` | Stops named as spoken ("open diapason", "principal four foot", "swell to great"). Ambiguous or unknown names change nothing and come back as a question |
| `general_cancel` | Push in every stop |
| `set_swell` | Expression pedal 0-100 % |
| `all_notes_off` | Silence immediately |
| `list_organs` / `select_organ` | Choose the organ definition |
| `start_recording` / `stop` / `play_recording` | The recorder (playing music by title arrives with the library) |

Tools act on the organ selected as **Organ out** (or the simulator) using its driver, and
read its state from MIDI traffic in both directions (`organ-core::live`), so toggle-only
stops (Makin/Johannus) are never flipped the wrong way. Replies are one short sentence,
suitable for reading aloud. Requests from other web pages are rejected (origin check);
set `REGISTRA_MCP_TOKEN` before listening on anything but localhost.
