"use client"

import * as React from "react"
import { FileMusicIcon, UploadIcon } from "lucide-react"
import { toast } from "sonner"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldSet,
  FieldLegend,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import {
  apiUrl,
  uploadFile,
  type DivisionRole,
  type ImportAnalysis,
  type ImportOptions,
  type TrackSummary,
  type XmlImportOptions,
} from "@/lib/api/client"
import { noteName } from "@/lib/midi"
import { formatClock } from "@/lib/time"

const DIVISIONS: { value: DivisionRole | "skip"; label: string }[] = [
  { value: "great", label: "Great" },
  { value: "swell", label: "Swell" },
  { value: "choir", label: "Choir" },
  { value: "solo", label: "Solo" },
  { value: "pedal", label: "Pedal" },
  { value: "skip", label: "Leave out" },
]

function DivisionSelect({
  value,
  onChange,
  label,
}: {
  value: string
  onChange: (v: string) => void
  label: string
}) {
  return (
    <Select
      value={value}
      items={DIVISIONS}
      onValueChange={(v) => v && onChange(String(v))}
    >
      <SelectTrigger className="w-32" aria-label={label}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          {DIVISIONS.map((d) => (
            <SelectItem key={d.value} value={d.value}>
              {d.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  )
}

/**
 * Import MIDI or MusicXML: pick a file, see what's in it, choose where each
 * part goes, then add it to the library.
 */
export function ImportDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
}) {
  const input = React.useRef<HTMLInputElement>(null)
  const [file, setFile] = React.useState<File | null>(null)
  const [analysis, setAnalysis] = React.useState<ImportAnalysis | null>(null)
  const [busy, setBusy] = React.useState(false)
  const [title, setTitle] = React.useState("")
  // MIDI
  const [organFormat, setOrganFormat] = React.useState(false)
  const [channels, setChannels] = React.useState<Record<number, string>>({})
  const [pedalUp, setPedalUp] = React.useState(false)
  const [split, setSplit] = React.useState(false)
  const [splitSeconds, setSplitSeconds] = React.useState(6)
  // MusicXML
  const [streams, setStreams] = React.useState<Record<string, string>>({})
  const [bpm, setBpm] = React.useState(80)
  const [verses, setVerses] = React.useState(1)

  const reset = () => {
    setFile(null)
    setAnalysis(null)
    setBusy(false)
  }

  async function choose(picked: File) {
    setFile(picked)
    setBusy(true)
    try {
      let f = picked
      if (/\.(png|jpe?g|tiff?|pdf)$/i.test(picked.name)) {
        // A scan: read the music first (this can take a minute).
        const res = await fetch(
          apiUrl(`/api/v1/omr?name=${encodeURIComponent(picked.name)}`),
          {
            method: "POST",
            body: picked,
            credentials: "include",
          }
        )
        if (!res.ok)
          throw new Error(
            (await res.json().catch(() => null))?.error ??
              "The scan couldn't be read"
          )
        f = new File(
          [await res.blob()],
          picked.name.replace(/\.[^.]+$/, ".mxl")
        )
        setFile(f)
      }
      const a = await uploadFile<ImportAnalysis>("/api/v1/import/analyse", f)
      setAnalysis(a)
      setTitle(a.title)
      if (a.midi) {
        setOrganFormat(a.midi.organ_match > 0.5)
        setChannels(
          Object.fromEntries(
            a.midi.channels.map((c) => [
              c.channel,
              c.duplicate_of ? "skip" : (c.suggested ?? "great"),
            ])
          )
        )
        setSplit(a.midi.gaps.length > 0)
      }
      if (a.musicxml) {
        setStreams(
          Object.fromEntries(a.musicxml.streams.map((s) => [s.id, s.suggested]))
        )
        setBpm(Math.round(a.musicxml.bpm))
        setVerses(1)
      }
    } catch (e) {
      toast.error((e as Error).message)
      setFile(null)
    } finally {
      setBusy(false)
    }
  }

  async function run() {
    if (!file || !analysis) return
    setBusy(true)
    try {
      let options: ImportOptions | XmlImportOptions
      if (analysis.kind === "midi") {
        const map: Record<string, DivisionRole> = {}
        for (const [ch, d] of Object.entries(channels))
          if (d !== "skip") map[ch] = d as DivisionRole
        options = {
          organ_format: organFormat,
          channels: organFormat ? {} : map,
          pedal_octave_up: pedalUp,
          split_ms: split ? splitSeconds * 1000 : null,
          title,
        }
      } else {
        const map: Record<string, DivisionRole> = {}
        const skip: string[] = []
        for (const [id, d] of Object.entries(streams)) {
          if (d === "skip") skip.push(id)
          else map[id] = d as DivisionRole
        }
        options = { streams: map, skip, bpm, verses, title }
      }
      const saved = await uploadFile<TrackSummary[]>(
        "/api/v1/import",
        file,
        options
      )
      toast.success(
        saved.length === 1
          ? `Imported “${saved[0].info.title}”`
          : `Imported ${saved.length} pieces`
      )
      onOpenChange(false)
      reset()
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const midi = analysis?.midi
  const xml = analysis?.musicxml

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        onOpenChange(o)
        if (!o) reset()
      }}
    >
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Import music</DialogTitle>
          <DialogDescription>
            MIDI files (including OrganAssist&apos;s library backup) or MusicXML
            from MuseScore, Sibelius and other notation programs.
          </DialogDescription>
        </DialogHeader>

        <input
          ref={input}
          type="file"
          accept=".mid,.midi,.musicxml,.xml,.mxl,.png,.jpg,.jpeg,.tif,.tiff,.pdf"
          className="sr-only"
          onChange={(e) => {
            const f = e.target.files?.[0]
            e.target.value = ""
            if (f) void choose(f)
          }}
        />

        {!analysis ? (
          <button
            type="button"
            onClick={() => input.current?.click()}
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => {
              e.preventDefault()
              const f = e.dataTransfer.files?.[0]
              if (f) void choose(f)
            }}
            className="flex flex-col items-center gap-3 rounded-xl border-2 border-dashed px-6 py-10 text-center transition-colors outline-none hover:bg-muted/50 focus-visible:ring-3 focus-visible:ring-ring/50"
            disabled={busy}
          >
            {busy ? (
              <Spinner />
            ) : (
              <UploadIcon
                className="size-8 text-muted-foreground"
                aria-hidden
              />
            )}
            <span className="font-medium">
              {busy
                ? `Reading ${file?.name}…`
                : "Choose a file, or drop it here"}
            </span>
            <span className="text-sm text-muted-foreground">
              {busy && /\.(png|jpe?g|tiff?|pdf)$/i.test(file?.name ?? "")
                ? "Reading the printed music. This can take a minute."
                : "MIDI (.mid), MusicXML (.musicxml, .mxl), or a scan or photo of printed music"}
            </span>
          </button>
        ) : (
          <div className="flex max-h-[60svh] flex-col gap-5 overflow-y-auto px-0.5">
            <div className="flex items-center gap-3 rounded-lg bg-muted/50 px-3 py-2 text-sm">
              <FileMusicIcon
                className="size-5 shrink-0 text-muted-foreground"
                aria-hidden
              />
              <span className="min-w-0 flex-1 truncate">{file?.name}</span>
              <span className="text-muted-foreground tabular-nums">
                {midi
                  ? formatClock(midi.duration_ms)
                  : xml
                    ? `${xml.measures} bars`
                    : ""}
              </span>
              <Button variant="ghost" size="sm" onClick={reset}>
                Change
              </Button>
            </div>

            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="import-title">Title</FieldLabel>
                <Input
                  id="import-title"
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                />
              </Field>
            </FieldGroup>

            {midi && (
              <>
                <FieldSet>
                  <FieldLegend variant="label">How was it made?</FieldLegend>
                  <RadioGroup
                    value={organFormat ? "organ" : "gm"}
                    onValueChange={(v) => setOrganFormat(v === "organ")}
                  >
                    <Field orientation="horizontal">
                      <RadioGroupItem value="organ" id="fmt-organ" />
                      <FieldLabel htmlFor="fmt-organ" className="font-normal">
                        Recorded on this organ, or an OrganAssist backup (keeps
                        the stops)
                      </FieldLabel>
                    </Field>
                    <Field orientation="horizontal">
                      <RadioGroupItem value="gm" id="fmt-gm" />
                      <FieldLabel htmlFor="fmt-gm" className="font-normal">
                        From other software: choose a keyboard for each channel
                      </FieldLabel>
                    </Field>
                  </RadioGroup>
                  {midi.organ_match > 0.5 && (
                    <FieldDescription>
                      {Math.round(midi.organ_match * 100)}% of its stop and
                      pedal messages match this organ.
                    </FieldDescription>
                  )}
                </FieldSet>

                {!organFormat && (
                  <FieldSet>
                    <FieldLegend variant="label">Channels</FieldLegend>
                    <ul className="flex flex-col divide-y rounded-lg border">
                      {midi.channels.map((c) => (
                        <li
                          key={c.channel}
                          className="flex flex-wrap items-center gap-3 px-3 py-2 text-sm"
                        >
                          <span className="w-24 font-medium">
                            Channel {c.channel}
                          </span>
                          <span className="min-w-0 flex-1 text-muted-foreground">
                            {[
                              c.name,
                              `${c.notes} notes`,
                              `${noteName(c.low)}–${noteName(c.high)}`,
                              c.polyphony >= 1.5 &&
                                `${c.polyphony.toFixed(1)} at once`,
                              c.duplicate_of &&
                                `same as channel ${c.duplicate_of}`,
                            ]
                              .filter(Boolean)
                              .join(" · ")}
                          </span>
                          <DivisionSelect
                            label={`Keyboard for channel ${c.channel}`}
                            value={channels[c.channel] ?? "skip"}
                            onChange={(v) =>
                              setChannels({ ...channels, [c.channel]: v })
                            }
                          />
                        </li>
                      ))}
                    </ul>
                    <Field orientation="horizontal">
                      <Checkbox
                        id="pedal-up"
                        checked={pedalUp}
                        onCheckedChange={(v) => setPedalUp(!!v)}
                      />
                      <FieldLabel htmlFor="pedal-up" className="font-normal">
                        Pedal notes an octave up (MuseScore writes pedal parts
                        at 16′ pitch)
                      </FieldLabel>
                    </Field>
                  </FieldSet>
                )}

                <Field orientation="horizontal" className="flex-wrap">
                  <Checkbox
                    id="split"
                    checked={split}
                    onCheckedChange={(v) => setSplit(!!v)}
                  />
                  <FieldLabel htmlFor="split" className="font-normal">
                    Split into separate pieces at silences of
                  </FieldLabel>
                  <Input
                    type="number"
                    min={2}
                    max={60}
                    value={splitSeconds}
                    onChange={(e) =>
                      setSplitSeconds(
                        Math.max(2, Math.min(60, Number(e.target.value) || 6))
                      )
                    }
                    className="w-20"
                    aria-label="Seconds of silence"
                    disabled={!split}
                  />
                  <span className="text-sm">seconds</span>
                  {midi.gaps.length > 0 && (
                    <FieldDescription className="w-full">
                      {midi.gaps.length === 1
                        ? "There is 1 long silence"
                        : `There are ${midi.gaps.length} long silences`}{" "}
                      in this file.
                    </FieldDescription>
                  )}
                </Field>
              </>
            )}

            {xml && (
              <>
                <FieldSet>
                  <FieldLegend variant="label">Parts</FieldLegend>
                  <ul className="flex flex-col divide-y rounded-lg border">
                    {xml.streams.map((s) => (
                      <li
                        key={s.id}
                        className="flex flex-wrap items-center gap-3 px-3 py-2 text-sm"
                      >
                        <span className="min-w-0 flex-1">
                          <span className="font-medium">{s.label}</span>
                          <span className="text-muted-foreground">
                            {" "}
                            · {s.notes} notes · {noteName(s.low)}–
                            {noteName(s.high)}
                          </span>
                        </span>
                        <DivisionSelect
                          label={`Keyboard for ${s.label}`}
                          value={streams[s.id] ?? "skip"}
                          onChange={(v) =>
                            setStreams({ ...streams, [s.id]: v })
                          }
                        />
                      </li>
                    ))}
                  </ul>
                  <FieldDescription>
                    The lowest voice is sent to the pedal; change it if
                    that&apos;s not right.
                  </FieldDescription>
                </FieldSet>
                <div className="flex flex-wrap gap-4">
                  <Field className="w-36">
                    <FieldLabel htmlFor="xml-bpm">
                      Tempo (beats a minute)
                    </FieldLabel>
                    <Input
                      id="xml-bpm"
                      type="number"
                      min={20}
                      max={300}
                      value={bpm}
                      onChange={(e) => setBpm(Number(e.target.value) || 80)}
                    />
                  </Field>
                  <Field className="w-36">
                    <FieldLabel htmlFor="xml-verses">Verses</FieldLabel>
                    <Input
                      id="xml-verses"
                      type="number"
                      min={1}
                      max={12}
                      value={verses}
                      onChange={(e) =>
                        setVerses(
                          Math.max(1, Math.min(12, Number(e.target.value) || 1))
                        )
                      }
                    />
                  </Field>
                </div>
              </>
            )}

            {midi &&
              !organFormat &&
              midi.channels.every((c) => channels[c.channel] === "skip") && (
                <Alert>
                  <AlertTitle>Every channel is left out</AlertTitle>
                  <AlertDescription>
                    Choose a keyboard for at least one channel.
                  </AlertDescription>
                </Alert>
              )}
          </div>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={run} disabled={!analysis || busy || !title.trim()}>
            {busy && analysis && <Spinner data-icon="inline-start" />}
            Import
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
