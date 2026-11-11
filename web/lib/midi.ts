const NOTE_NAMES = ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"]

/** MIDI note number to scientific pitch name (60 = C4). */
export function noteName(note: number): string {
  return `${NOTE_NAMES[note % 12]}${Math.floor(note / 12) - 1}`
}

export type MidiKind =
  | "note-on"
  | "note-off"
  | "control"
  | "program"
  | "pitch-bend"
  | "pressure"
  | "sysex"
  | "system"
  | "unknown"

export interface DecodedMidi {
  kind: MidiKind
  /** 1-16, absent for system messages. */
  channel?: number
  summary: string
}

/** Human-readable description of one raw MIDI message. */
export function describeMidi(bytes: number[]): DecodedMidi {
  const [status = 0, d1 = 0, d2 = 0] = bytes
  const channel = (status & 0x0f) + 1

  switch (status & 0xf0) {
    case 0x80:
      return { kind: "note-off", channel, summary: `Note off ${noteName(d1)}` }
    case 0x90:
      // Note-on with velocity 0 is a note-off by convention.
      return d2 === 0
        ? { kind: "note-off", channel, summary: `Note off ${noteName(d1)}` }
        : { kind: "note-on", channel, summary: `Note on ${noteName(d1)} vel ${d2}` }
    case 0xa0:
      return { kind: "pressure", channel, summary: `Key pressure ${noteName(d1)} ${d2}` }
    case 0xb0:
      return { kind: "control", channel, summary: `CC ${d1} = ${d2}` }
    case 0xc0:
      return { kind: "program", channel, summary: `Program ${d1 + 1}` }
    case 0xd0:
      return { kind: "pressure", channel, summary: `Channel pressure ${d1}` }
    case 0xe0:
      return {
        kind: "pitch-bend",
        channel,
        summary: `Pitch bend ${((d2 << 7) | d1) - 8192}`,
      }
  }
  if (status === 0xf0) {
    return { kind: "sysex", summary: `SysEx (${bytes.length} bytes)` }
  }
  if (status >= 0xf0) {
    return { kind: "system", summary: `System 0x${status.toString(16)}` }
  }
  return { kind: "unknown", summary: "Unknown" }
}

export function toHex(bytes: number[]): string {
  return bytes.map((b) => b.toString(16).padStart(2, "0").toUpperCase()).join(" ")
}

/** mm:ss.t */
export function formatDuration(ms: number): string {
  const totalTenths = Math.floor(ms / 100)
  const tenths = totalTenths % 10
  const seconds = Math.floor(totalTenths / 10) % 60
  const minutes = Math.floor(totalTenths / 600)
  return `${minutes}:${String(seconds).padStart(2, "0")}.${tenths}`
}
