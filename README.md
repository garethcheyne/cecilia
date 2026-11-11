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

## What it does

- **Records a whole service with one press.** Each piece is saved to the
  library by itself, with the stops that were drawn when it started.
- **Plays any recording on any organ.** The virtual organist maps
  registrations onto the stops the connected organ has (never louder).
  Tempo, verse control ("stop at the end of this verse", "skip to the last
  verse"), latching pause, seek and fade-out.
- **Runs services** from playlists: each piece waits for Play, starts by
  itself after a pause, or is cued at its first note. A large-text Service
  screen is meant for whoever runs the music on the day.
- **Edits recordings** in a piano roll (notes, stop changes, swell pedal,
  verse markers, sections, undo) and shows them as **staff notation**.
- **Imports and exports** MIDI (including OrganAssist's "Backup Entire Library
  as MIDI"), MusicXML from notation programs, and **scanned printed music**
  through Audiveris (see [docs/OMR.md](docs/OMR.md)).
- **Learns your organ**: copy a template (Makin, demo), then MIDI-learn
  every stop, keyboard, swell pedal and the general cancel at the console.
- Ships with **eleven sample pieces**: public-domain hymn tunes (Old
  Hundredth, Eventide, St Anne, Crimond…) and an original prelude.
- A built-in **simulator** with sound, so all of this works without an
  organ.

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

On a Raspberry Pi, install the `.deb` instead: see
[docs/RASPBERRY-PI.md](docs/RASPBERRY-PI.md).

## Tests

```sh
node scripts/test-all.mjs            # everything: Rust, web unit, browser tests
node scripts/test-all.mjs --skip-e2e # without the browser tests
```

- **Rust**: unit tests in `organ-core` (drivers, recorder, player timing,
  virtual organist, MIDI/MusicXML files, samples) and integration tests in
  `organ-server/tests` that start a real server on a spare port and drive
  the simulator over HTTP.
- **Web**: Vitest for the editing, notation, MIDI-learn and parsing logic
  (`web/lib/*.test.ts`); Playwright browser tests (`web/e2e`) on desktop and
  phone sizes using the installed Edge or Chrome.
- CI runs the lot on every push (`.github/workflows/ci.yml`).

## Changing the API

The Rust types are the source of truth. After changing them:

```sh
cd web && npm run gen:api        # regenerates lib/api/openapi.json + schema.d.ts
```

## Configuration (environment variables)

| Variable            | Default                                         | Purpose                                   |
| ------------------- | ----------------------------------------------- | ----------------------------------------- |
| `REGISTRA_ADDR`     | `127.0.0.1:7070`                                | Listen address. Use `0.0.0.0:7070` for LAN access, after setting a password |
| `REGISTRA_DATA_DIR` | OS data dir + `/Registra`                       | Settings, library, playlists, organs, history |
| `REGISTRA_WEB_DIR`  | `./web` beside the binary, else `../web/out`    | Static UI to serve                        |
| `REGISTRA_CORS`     | `http://localhost:3000,http://127.0.0.1:3000`   | Browser origins allowed to call the API   |
| `REGISTRA_AUDIO`    | on                                              | `0` keeps the simulator silent            |
| `REGISTRA_OMR`      | Audiveris on the PATH                           | OMR command with `{input}` and `{output}` |
| `REGISTRA_MCP_TOKEN`| unset                                           | If set, `/mcp` requires `Authorization: Bearer <token>` |
| `RUST_LOG`          | `info`                                          | Log filter                                |

## Data folder

```
settings.json                  MIDI ports, recording/playback settings, password hash
library/<id>.track.json        recordings, samples and imports (organ-neutral)
playlists/<id>.playlist.json   orders of service
organs/<id>.organ.json         your organ definitions
history/<id>/…                 the last 20 versions of each track
```

All JSON, documented by the OpenAPI schema at `/api/v1/openapi.json`.
Settings → Backup downloads all of it as one zip, and restores it.

## API (v1)

The full, typed list is the OpenAPI document (`/api/v1/openapi.json`). In
outline:

| Area | Paths |
| --- | --- |
| MIDI | `/midi/ports`, `/midi/config`, `/midi/status`, `/midi/panic`, `/midi/test/*` |
| Organs | `/organs`, `/organs/{id}`, `/organs/copy`, `/organs/{id}/save`, `/organs/{id}/delete` |
| Simulator | `/simulator`, `/simulator/organ`, `/simulator/act`, `/simulator/reset` |
| Live organ | `/live`, `/live/act` |
| Library | `/library`, `/library/{id}` (GET/PUT/PATCH/DELETE), `/library/{id}/duplicate`, `/library/{id}/mapping`, `/library/{id}/midi` |
| Recorder | `/recorder`, `/recorder/start`, `/recorder/stop` |
| Player | `/player`, `/player/load`, `/player/play`, `/player/pause`, `/player/stop`, `/player/seek`, `/player/tempo`, `/player/verses`, `/player/fade` |
| Playlists | `/playlists`, `/playlists/{id}`, `/playlists/{id}/duplicate`, `/verses/parse` |
| Service | `/service`, `/service/start`, `/service/next`, `/service/previous`, `/service/select`, `/service/end` |
| Files | `/import/analyse`, `/import`, `/omr`, `/export/library.csv`, `/export/library-midi.zip`, `/export/backup.zip`, `/import/backup` |
| System | `/health`, `/settings`, `/auth`, `/auth/login`, `/auth/logout`, `/auth/password`, `/assistant` |
| Events | `/events` (WebSocket: `status`, `midi`, `simulator`, `live`, `player`, `recorder`, `service`, `library`) |
| Assistant | `/mcp` (outside `/api/v1`) |

## Targets

`organ-core` uses `midir` (WinMM on Windows, CoreMIDI on macOS, ALSA on Linux),
so the server builds for x64 and ARM64 on Windows, macOS and Linux. Tagged
releases build Raspberry Pi packages (`.github/workflows/release.yml`).

## Roadmap

The full feature spec is in [docs/SPEC.md](docs/SPEC.md).

1. ✅ MIDI devices, live monitor, raw test record/playback
2. ✅ Organ definitions, simulator, live console, Makin support
3. ✅ Auto-recorder, library, sample music
4. ✅ Playback engine and virtual organist
5. ✅ Library UI, playlists and services, MIDI/MusicXML import and export
6. ✅ Track editor and score view
7. ✅ Organ designer and MIDI learn (combination pistons and more drivers still to come)
8. ✅ Settings, backups, password, Raspberry Pi packaging (translations still to come)
9. ✅ Scanned music (OMR) through Audiveris
10. Assistant: music tools and voice control (paused)

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
