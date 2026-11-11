"use client"

import * as React from "react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Slider } from "@/components/ui/slider"
import { api, call, type TrackSummary } from "@/lib/api/client"

const FIELDS = [
  { key: "title", label: "Title", placeholder: "" },
  { key: "composer", label: "Composer", placeholder: "e.g. W. H. Monk" },
  { key: "melody", label: "Tune name", placeholder: "e.g. EVENTIDE" },
  { key: "performer", label: "Played by", placeholder: "" },
  { key: "publication", label: "Publication", placeholder: "e.g. Hymns Ancient & Modern 13" },
  { key: "copyright", label: "Copyright", placeholder: "" },
] as const

type Draft = Record<(typeof FIELDS)[number]["key"], string> & { volume: number }

function draftOf(t: TrackSummary): Draft {
  const i = t.info
  return {
    title: i.title,
    composer: i.composer ?? "",
    melody: i.melody ?? "",
    performer: i.performer ?? "",
    publication: i.publication ?? "",
    copyright: i.copyright ?? "",
    volume: i.volume ?? 100,
  }
}

/** Edit a track's title, credits and playback volume. */
export function TrackDetailsDialog({
  track,
  open,
  onOpenChange,
}: {
  track: TrackSummary | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const [draft, setDraft] = React.useState<Draft | null>(null)
  const [saving, setSaving] = React.useState(false)

  // Start each opening from the track as it is now.
  const [shownFor, setShownFor] = React.useState<string | null>(null)
  if (open && track && shownFor !== track.id) {
    setShownFor(track.id)
    setDraft(draftOf(track))
  }
  if (!open && shownFor !== null) setShownFor(null)

  const invalid = !draft || draft.title.trim() === ""

  async function save(e: React.FormEvent) {
    e.preventDefault()
    if (!track || !draft || invalid) return
    setSaving(true)
    try {
      await call(api.PATCH("/api/v1/library/{id}", { params: { path: { id: track.id } }, body: draft }))
      toast.success("Details saved")
      onOpenChange(false)
    } catch (err) {
      toast.error((err as Error).message)
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <form onSubmit={save} className="flex flex-col gap-6">
          <DialogHeader>
            <DialogTitle>Track details</DialogTitle>
            <DialogDescription>Shown in the library, in playlists and when it plays.</DialogDescription>
          </DialogHeader>
          {draft && (
            <FieldGroup className="max-h-[60svh] overflow-y-auto px-0.5">
              {FIELDS.map((f) => (
                <Field key={f.key} data-invalid={(f.key === "title" && invalid) || undefined}>
                  <FieldLabel htmlFor={`track-${f.key}`}>{f.label}</FieldLabel>
                  <Input
                    id={`track-${f.key}`}
                    value={draft[f.key]}
                    placeholder={f.placeholder}
                    onChange={(e) => setDraft({ ...draft, [f.key]: e.target.value })}
                    aria-invalid={(f.key === "title" && invalid) || undefined}
                    required={f.key === "title"}
                  />
                </Field>
              ))}
              <Field>
                <FieldLabel htmlFor="track-volume">Volume: {draft.volume}%</FieldLabel>
                <Slider
                  id="track-volume"
                  min={1}
                  max={100}
                  value={draft.volume}
                  onValueChange={(v) => setDraft({ ...draft, volume: v as number })}
                />
                <FieldDescription>Applied through the organ&apos;s master volume while this track plays.</FieldDescription>
              </Field>
            </FieldGroup>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={saving || invalid}>
              Save details
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
