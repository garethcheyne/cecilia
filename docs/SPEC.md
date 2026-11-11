# Cecilia – Functional Specification

**Sources:** the OrganAssist 3.3 user manual (bundled in the installer) and
organassist.com (Home, Features, Release Notes, FAQ, Allen, Videos, History pages,
read Nov 2026). They describe *behaviour*. We reimplement that behaviour from
scratch; we never use OrganAssist's code, binaries or encrypted file formats.

## 1. What we're building

An organ-specific MIDI sequencer for classical/church organists who don't want to
know about MIDI. It:

1. **Records a whole service with one click.** Each piece becomes its own track,
   and the registration in use is captured at the start of every track.
2. **Plays any recording on any organ.** Tracks are stored in a neutral form
   (notes, stop changes, expression), and MIDI is generated live for whichever
   organ is connected, substituting stops where needed ("virtual organist").
3. **Edits performances graphically.** A piano roll for notes, stops, expression,
   sections, tempo and hymn verses, with no MIDI knowledge needed.
4. **Runs services.** Playlists with verse control, cueing, delays and fades,
   driven by the space bar.

### Modernisation goals (gaps in the original)

| Original limitation | Cecilia |
| --- | --- |
| Windows x86 only. "Not ARM", macOS dropped | Windows, macOS and Linux on x64 and ARM64. Raspberry Pi as a console box |
| VB6 desktop UI, "dated" (author's word) | Web UI, usable from a laptop, tablet or phone on the LAN |
| Encrypted proprietary files | Open JSON formats, documented and diff-able |
| Settings via hand-editing `OrganAssist.ini` | Every setting in the UI and the API |
| "VERY BASIC" sound-card fallback | Built-in **organ simulator** with proper synthesis |
| One process, one window per organ | Headless server, any number of clients, scripting via API |
| Backups as .bak renames and zip | Versioned history per track and organ, one-click restore |

### Primary deployment target: Raspberry Pi / SBC

Development happens on Windows, but the end product is a small box (Raspberry
Pi 4/5, Zero 2 W, or a similar SBC) living at the organ:

- **Builds:** `aarch64-unknown-linux-gnu` (64-bit Pi OS) and
  `armv7-unknown-linux-gnueabihf` (32-bit). MIDI via ALSA, using USB-MIDI
  adapters (ESI/Roland recommended) or a GPIO UART DIN interface.

- **Packaging:** runs headless as a systemd service, packaged as a `.deb`.
  Optionally boots straight into **auto-record**, so the whole service is
  captured with nobody touching anything.

- **Control:** from a tablet or phone on the church Wi-Fi, or a Chromium kiosk
  on a Pi touchscreen. Authentication is required before LAN exposure.

- **Simulator audio:** rendered by the server (Rust synth → ALSA/WASAPI), so
  the box itself can drive speakers. A browser client can't produce sound on
  the Pi.

- **Budget:** low RAM and CPU. Synth uses wavetables, not per-partial
  oscillators. Avoid SD-card wear (atomic writes, no chatty logging to disk).
  The UI is static files.

## 2. Concepts

- **Organ definition.** Divisions (keyboards), stops, couplers, tremulants,
  combinations, expression pedals, master volume, the screen layout, and the MIDI
  bindings for each. `manufacturer` selects a **driver** that knows that maker's
  MIDI dialect (the "printer driver" idea).

- **Division roles** (fixed, English convention): Pedal, Great, Swell, Choir,
  Solo, Echo/Bombarde, up to 5 manuals + pedal. Each division has a display name
  (Hauptwerk, GO, …), a key range (MIDI low/high) and optional expression.
  Fixed role colours: Pedal purple, Choir cyan, Great blue, Solo red, Swell
  green, 5th manual gold.

- **Stop equivalent.** A structured classification (division role, family,
  pitch/footage, name tag). It must be unique within an organ. It is the only
  thing a track stores about a stop, which makes tracks portable.

- **Track.** A neutral recording. Header: metadata, organ recorded on, initial
  registration and expression, margins, verse markers, tempo map. Events: note
  on/off (division, key, velocity), stop change (equivalent, on/off),
  expression (division, 0–127), master volume.

- **Library.** All tracks plus metadata. **Playlist.** An ordered list of tracks
  and text lines with per-row playback options.

## 3. Features

Phase numbers refer to §5.

### 3.1 MIDI setup & diagnostics (P1 ✅)

- Pick several inputs and one output. Changes apply immediately.
- Raw test recorder and player that bypasses all logic (hardware check).
- Live MIDI monitor, newest at the bottom (original) or top (ours), decoded.
- **Sound card / simulator option.** Explicitly *not persisted* in the original
  ("to avoid any potential for error in a live church service"). Keep that
  safety rule for the "send playback to simulator instead of the organ" switch.

- MIDI feedback protection (ignore our own echoed output), so notes don't stick.
  This is a user option.

### 3.2 Organ definitions & designer (P2)

- **Organ picker** with thumbnails. "Use as default" and "don't prompt on load"
  options.

- **Designer** (edit layout), with save and save-as (copy an existing organ as
  the starting point):

  - Add stop, keyboard or label. A new stop copies the nearest one. Delete.
  - Select, move, resize, grid (+/- size), hide keyboards for space.
  - Box-select. Then copy stop details (size and colours) and paste size or
    colour to the selection or to "similar" stops. Auto-align (diagonal, grid,
    terraced, tab), move group, zoom group (positions only; with Shift, sizes
    too).

  - Stop properties: 3 name lines + full name (tooltip, auto-built), division,
    equivalent (searchable: "16b" jumps to 16′ B…, "CG" to couplers G…), text and
    background colour, invert colours, shape (round, rectangle), repeat-ms
    (conflicting on/off spacing for moving drawstops), delay-ms (pause after the
    change).

  - Keyboards: rename, reposition, MIDI low/high note. Coupler equivalents are
    generated from the existing divisions and renamed with them. Tracker couplers
    are opt-in.

  - Multi-level undo/redo. Autosave every 30 s with crash recovery.
- **MIDI learn.** A wizard: play notes on each keyboard, move each expression
  pedal through its range, toggle every stop on and off in turn. Single-item
  re-learn. Problems listed on save. "Clear ALL MIDI". Manual entry of hex
  codes.

  - Up to **5 bindings per stop** (Hauptwerk jambs, crescendo, thumb and foot
    pistons). Types: on/off pair, toggle, value-slot pattern (the original's FF
    marker), "double/reverse" message pairs (the FD marker), NRPN (with an
    optional CC 38), SysEx.

  - Master volume: pattern + value range, set by hand.
- **Combinations.** Divisional and general pistons. General Cancel.
  - Hauptwerk: Master Couplers, and Master General / Scoped combinations with
    "default MIDI for HW". A combination array is auto-numbered when you add
    the next one beside it.

  - Allen mode: internal combinations (99 memory levels, set by right-click,
    clear a level), "divisionals include couplers", "force combinations onto
    organ".

- **Multi-screen layouts.** Several layouts share one organ. In Cecilia that's
  just several browser windows or devices, each with its own view.

- **Drivers:** Generic, Hauptwerk 4.2+, Johannus/Makin (pre-2000 and
  programmable), Rodgers (current, including MIDI Voices A/B in the track header;
  Phase II), Peterson (SysEx swell preferred), Solid State (type 1 `F0 2B 01 01`,
  type 2 `F0 2B 01 02`), Viscount (tremulant speed/depth), Walker, Wyvern,
  Content, Classic Organ Works, Allen. Generic, Hauptwerk and **Makin** first (the target church has a Makin). The rest are
  table-driven and added as users with those organs help verify them.

### 3.3 Organ console window (P2)

- Live console showing stops, keyboards and expression. Click a stop or a note
  to operate it. It resizes dynamically, abbreviates names to fit, and lets you
  pick the font size, typeface and bold.

- Reset menu: clear all stops (with or without sending MIDI), reset the default
  expression.

- **Real-time translation.** Load two organs, so input from organ A is played
  on organ B live.

### 3.4 Simulator (P2) – new

A virtual organ built into the server, so everything works with no hardware:

- It appears as a normal MIDI port pair ("Cecilia Simulator"). It drives an
  organ definition: messages sent to it update its stop, note and expression
  state; clicks and keys on the web console are encoded with that organ's real
  bindings and arrive as input. This exercises the whole pipeline honestly.

- **Sound.** Web Audio synthesis in the browser. Each stop has a family timbre
  (principal, flute, string, celeste, reed, mutation, mixture) at its footage.
  Expression drives gain, tremulants drive LFO. You can play it from the
  on-screen keyboard, the computer keyboard or any Web MIDI keyboard.

- Uses: practise and edit at home, demo, automated tests, developing drivers
  for organs we don't own.

### 3.5 Recording (P3)

- Record button: the recorder waits for the first note and then starts a track
  with the current registration and expression. After N seconds of silence
  ("recording delay", user option) it saves the track and keeps monitoring.
  Tracks are named by date and time.

- Background recording while playing back (records only console input). It
  stops automatically if a track is edited.

- After a session, playback defaults to the last recorded track.
- Default margins per organ (1–15 s of silence before the first note and after
  the last).

### 3.6 Playback (P4)

- Transport: record, previous, rewind (audible scrub), play, pause (latching,
  "tape recorder" style), fast-forward, next, stop. Elapsed and remaining time
  (over 1 h supported).

- Live MIDI generation for the current organ, including **reverse
  regeneration when rewinding**, and coping with manual stop changes during
  playback.

- **Virtual organist:** maps each recorded registration onto the stops this
  organ has, as a whole registration and not stop by stop. It never substitutes
  something wildly louder ("never replace your Salicional with a Tuba"). Notes
  for missing manuals are redirected (ultimately to the Great).

- Tempo slider (double-click resets to 100 %). Master volume slider (if
  configured). Motorised fade-out over the fade-duration option: stop at zero,
  then restore the volume. Click again to cancel.

- Verse control: number of verses, "stop at end of this verse", "skip to last
  verse" (both wait for the verse end and can be cancelled). Shows current/total
  (2/4).

- Per-track volume (1–100) applied for the duration of the track.
- Repeat-ms and delay-ms honoured. Optional "force volume" for noisy expression
  pedals.

- Auto-pause before the first and after the last note (for audio recording).
- Track view (read-only): follow the playback, double-click to start or stop
  there, show verse labels, smooth or paged scrolling.

- Keyboard shortcuts (configurable): record, previous, play, next, stop, pause,
  quit at verse end, last verse, fade.

- Accuracy target from the original's stress test: 99.9 % of notes within 9 ms,
  stop changes within 100 ms.

### 3.7 Library (P5)

- A table of tracks: title, composer, performer, publication, copyright, melody
  (editable); volume; duration, verses, organ (read-only). Sort by any column,
  search forwards and backwards, double-click to play.

- Right-click: edit track, delete, copy, split track (only on the organ it was
  recorded on), edit field, edit MIDI data (e.g. Rodgers voice).

- Export the library as CSV.
- **MIDI import:**
  - For the current organ: keeps registration and expression, imports several
    files at once, scans for gaps, offers to split into pieces (with margins),
    re-export them, and delete temporary files.

  - General MIDI: per-channel analysis (low and high note, average polyphony,
    duplicate-channel detection), map channels to divisions (several channels
    to one division allowed), remembered defaults, or "use export settings".

  - MuseScore: assigns staves to channels 1–3 and fixes pedal octave
    (16′-stored) notes. Remembers its own defaults.

- **MIDI export:** current organ format (General Cancel first) or General MIDI
  with configurable division→channel mapping. Backup entire library as MIDI.

### 3.8 Playlists (P5)

- Insert tracks from the Library (several at once, in order). Blank and text
  lines. Create from a blank template.

- Per row:
  - **Delay**: blank = wait for space/play; a number ≥ 0 = auto-continue after N
    seconds; P/C = cue at the first note and wait.

  - **Verses**: a count (< 20) or an explicit list ("12357", repeats allowed).
- Two narrative columns: free text, or bound to composer, performer,
  publication, copyright, melody or volume.

- Repeat, shuffle, **silent mode** (don't reset stops at the end of a track;
  reset when the next one starts or the playlist closes).

- Safety: warn when a track was edited after it was added to the playlist, until
  the playlist is re-saved.

- Large-text mode for whoever operates it during a service.

### 3.9 Track editor (P6)

- Layout: registration and expression lane on top, piano roll below, margins and
  verse markers under the divider. Transport at the bottom. Fixed division
  colours. Solid or hatched notes. Zoom (window width in minutes and seconds).
  Smooth or paged scrolling. Stop tooltips optionally in the stop's own colours.
  The stack height of simultaneous stop changes is configurable (8–50, default
  15).

- **Notes:**
  - Drag vertically to change pitch, horizontally to change time, drag an edge
    to change length.

  - Drag on empty space to add a note, then choose its division.
  - Move a note to another keyboard.
- **Selection** (by time and pitch):
  - Delete, copy and paste (to one division or all).
  - Move or swap between keyboards.
  - Transpose ±1 semitone (repeatable).
- **Stops:**
  - Markers are solid when a stop comes on and show an X when it goes off.
    Stacked when simultaneous. Hover for the name.

  - Drag to move, delete, edit (swap for another stop in the same division;
    filter <8′/8′/>8′ on large organs).

  - Add a stop change. It toggles relative to the state at that point.
  - Or play the track and change stops live, so the changes get recorded.
- **Expression:**
  - The expression line is drawn in the top lane.
  - Adjust a volume with a slider. Live slider moves during playback are
    relative to the recorded values.

  - Smooth volumes removes noise from the curve.
  - Insert the default expression settings.
- **Sections** (selected in the top lane):
  - Copy, then insert elsewhere. The clipboard survives switching tracks, so
    you can splice pieces from different recordings.

  - Delete notes, stops or expression, or delete all events.
  - Delete but keep the registration and expression state.
  - Insert blank time.
  - Transpose.
  - Select the entire track, start→cursor, or cursor→end.
- **Whole track:**
  - Undo (multi-level), save, save as.
  - Transpose ±1.
  - Include tempo changes (the tempo slider moves during playback are baked in
    on stop).

- **Margins** are draggable. Default margins per organ.
- **Hymn verses:** add, move or delete verse markers (yellow), placed in the
  silence between verses.

- **Mute/solo per division** during playback. Overdub notes, expression and
  registration during playback.

- Double-click to start playback at a point. Pause positions you on the first
  note to change the initial registration.

- Autosave every 30 s with crash recovery. A version saved on each save.

### 3.9a Score view – new (P6)

- Show a track as **staff notation**: grand staff per manual, plus a pedal
  staff, with clefs, key and time signatures, bar lines and ties. Follows the
  playback cursor, as the piano roll does.

- Exact for tracks that come from symbolic sources (bundled samples, MusicXML
  import). Live recordings are quantised to a beat grid (tempo detection plus
  a user-adjustable grid), so their notation is approximate.

- Rendering via VexFlow (MIT) in the browser. Optional MusicXML export so
  MuseScore can open recordings.

### 3.9b Sample music – new (P3/P4)

- A bundled library of original organ pieces and arrangements of
  public-domain hymn tunes (e.g. Old Hundredth, Bach-style chorales), with
  registration changes, expression and verse markers.

- Authored in a small symbolic format (notes with beat durations, parts →
  divisions, tempo, key and metre) that compiles to a normal track. They act as
  demo content, test fixtures for playback, editing and the virtual organist,
  and exact sources for the score view.

### 3.9c Scanned music (OMR) – new (P9)

- Upload a scan, photo or PDF of printed music. Optical music recognition
  turns it into MusicXML, which is then imported like any score.

- Pipeline: image → OMR engine (Audiveris, AGPL, run as a separate process
  and never linked in; pluggable so other engines can be swapped in) →
  MusicXML → **review in the score view** (fix wrong notes and rhythms) →
  assign staves to manuals and pedal (as for MuseScore import) → track.

- Needs MusicXML import (P5) first. Heavy for small SBCs, so it runs on a Pi 5
  or desktop, or as an optional "scan service" on the LAN. The core server
  stays light.

- Expect good results on clean printed hymnals. Handwriting and poor copies
  will need manual correction.

### 3.10 MIDI translation mode (P7)

- Rule-based MIDI filter: input pattern with wildcards → output pattern, with
  calculations (mask, add, subtract per wildcard byte). Several rules can match.
  Unmatched messages pass through unchanged. Not for recording or playback.
  The wildcard/calculation semantics are well specified in the manual. We can
  offer a clearer UI on the same model.

### 3.11 System (across phases)

- **Settings UI:** language, text size and font, recording delay, fade duration,
  reverse scroll, auto-pause, ignore MIDI feedback, default master volume,
  default playlist delay, NRPN CC 38 per organ, force volume per organ.
  Simulator voices replace the original's `[SoundCard]` GM program numbers.

- **Localisation:** English first. The original ships English, French and
  Portuguese.

- **Data and backups:** one data folder, history versions for every track and
  organ edit, periodic full backup archive, restore from the UI.

- **Multiple configurations:** profiles (separate data folders and defaults)
  replace numbered shortcuts.

- **Migration from OrganAssist:** import the MIDI files from its "Backup Entire
  Library as MIDI" (current organ format). Users rebuild their organ definition
  once with MIDI learn, or start from an included template.

- **Security** (new, because the server is networked): login and roles
  (operator: playlist and transport only; editor; admin) before LAN binding is
  encouraged.

## 4. File formats (open JSON)

- `organs/<id>.organ.json`: definition, layout, bindings, defaults.
- `library/<id>.track.json`: header + event list (times in ms from the track
  start; tempo map separate). Gzip when large.

- `playlists/<id>.playlist.json`
- `settings.json`, `history/…` (versions), `backups/…`

All are versioned with a `schema` number and migrated on load.

## 5. Phases

| # | Deliverable | Status |
| --- | --- | --- |
| P1 | MIDI ports, monitor, raw test record/play | ✅ |
| P2 | Organ model + API, demo organs, **simulator** (virtual port + console UI + server-side synth), live console. Makin/Johannus toggle stops + general cancel, built-in Makin template | ✅ |
| P3 | Auto-recorder → neutral tracks in the library; 11 bundled sample pieces | ✅ |
| P4 | Playback engine: live generation, virtual organist, transport, tempo, verses, fade, volume. On organs with toggle-only stops (Makin), send `all_stops_off` and wait for it to settle before setting the opening registration | ✅ |
| P5 | Library UI, playlists, MIDI import/export (GM, MuseScore, current organ, split), CSV; MusicXML import | ✅ |
| P6 | Track editor (piano roll) with undo, sections, verses; score view. Still to come: tempo bake, mute/solo, overdub | ✅ (core) |
| P7 | Organ designer + MIDI learn wizard (**Makin first**: the target church has one), combinations, Hauptwerk extras; drivers beyond Generic/Hauptwerk/Makin; translation mode | ✅ designer + MIDI learn (combinations, extra drivers, translation mode still to come) |
| P8 | Settings UI, i18n, backups/history, profiles, auth, installers for all six targets | ✅ settings, backups, history, password, Pi .deb + release builds (i18n, profiles, roles still to come) |
| P9 | Scanned music: OMR (Audiveris) → MusicXML → review → track | ✅ |
| A1 | **Assistant (MCP server)** at `/mcp`: stops by spoken name, swell, cancel, silence, organ select, record/stop/play; assistant name setting | ✅ |
| A2 | Assistant music tools with P4/P5: `find_music`, `play_music` (title, verses, tempo), `pause`/`resume`/`next` in a playlist | |
| A3 | **Voice control** on the Pi: local wake word (the assistant's name, default **Cecilia**; built after P4 so "play …" works from day one) + local speech-to-text, an LLM calling the A1/A2 tools, spoken or on-screen confirmation; push-to-talk option for services | |

Design note: the domain logic (track model, edit operations, virtual organist,
driver encode/decode) lives in **`organ-core`** with unit tests. The UI never
re-implements it; edits go through the API as operations, which also gives
server-side undo history.
