"use client"

import * as React from "react"
import { SendIcon } from "lucide-react"

import {
  AccountGate,
  ShareDone,
  ShareProblem,
  useCommunityAccount,
} from "@/components/library/share-common"
import { MANUFACTURERS } from "@/components/organs/organs-index"
import { Button } from "@/components/ui/button"
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
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import {
  api,
  call,
  type OrganDefinition,
  type ShareResult,
} from "@/lib/api/client"

type Draft = {
  maker: string
  model: string
  series: string
  variant: string
  years: string
  notes: string
  note: string
}

function draftOf(organ: OrganDefinition): Draft {
  const maker =
    organ.manufacturer === "generic"
      ? ""
      : (MANUFACTURERS[organ.manufacturer] ?? "").replace(/\s*\(.*\)$/, "")
  return {
    maker,
    model: organ.name,
    series: "",
    variant: "",
    years: "",
    notes: (organ.connection?.notes ?? []).join("\n"),
    note: "",
  }
}

/** Share a saved organ setup with other churches. */
export function ShareOrganDialog({
  organ,
  open,
  onOpenChange,
}: {
  organ: OrganDefinition
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { account, problem: accountProblem } = useCommunityAccount(open)
  const [draft, setDraft] = React.useState<Draft>(() => draftOf(organ))
  const [busy, setBusy] = React.useState(false)
  const [problem, setProblem] = React.useState<string | null>(null)
  const [result, setResult] = React.useState<ShareResult | null>(null)

  // Start afresh each time it opens.
  const [wasOpen, setWasOpen] = React.useState(open)
  if (open !== wasOpen) {
    setWasOpen(open)
    if (open) {
      setDraft(draftOf(organ))
      setProblem(null)
      setResult(null)
    }
  }

  const learned = organ.stops.filter((s) => (s.midi ?? []).length > 0).length
  const keyboards = organ.divisions.length
  const ready = draft.maker.trim() !== "" && draft.model.trim() !== ""
  const set = (k: keyof Draft) => (v: string) => setDraft({ ...draft, [k]: v })

  async function send(e: React.FormEvent) {
    e.preventDefault()
    if (!ready) return
    setBusy(true)
    setProblem(null)
    try {
      const r = await call(
        api.POST("/api/v1/community/share/organ/{id}", {
          params: { path: { id: organ.id } },
          body: {
            maker: draft.maker.trim(),
            model: draft.model.trim(),
            series: draft.series.trim(),
            variant: draft.variant.trim(),
            years: draft.years.trim(),
            notes: draft.notes
              .split("\n")
              .map((l) => l.trim())
              .filter(Boolean),
            note: draft.note.trim(),
          },
        })
      )
      setResult(r)
    } catch (err) {
      setProblem((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <form onSubmit={send} className="flex flex-col gap-6">
          <DialogHeader>
            <DialogTitle>Share with other churches</DialogTitle>
            <DialogDescription>
              {keyboards} {keyboards === 1 ? "keyboard" : "keyboards"},{" "}
              {organ.stops.length} stops, {learned} learned. Organ setups are
              shared freely (CC0) for anyone to use.
            </DialogDescription>
          </DialogHeader>

          {result ? (
            <ShareDone result={result} />
          ) : (
            <AccountGate account={account} problem={accountProblem}>
              <FieldGroup className="max-h-[55svh] overflow-y-auto px-0.5">
                <div className="grid gap-4 sm:grid-cols-2">
                  <Field>
                    <FieldLabel htmlFor="share-maker">Maker</FieldLabel>
                    <Input
                      id="share-maker"
                      value={draft.maker}
                      onChange={(e) => set("maker")(e.target.value)}
                      placeholder="e.g. Johannus"
                      required
                    />
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="share-model">Model</FieldLabel>
                    <Input
                      id="share-model"
                      value={draft.model}
                      onChange={(e) => set("model")(e.target.value)}
                      placeholder="e.g. Studio 170"
                      required
                    />
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="share-series">Series</FieldLabel>
                    <Input
                      id="share-series"
                      value={draft.series}
                      onChange={(e) => set("series")(e.target.value)}
                      placeholder="e.g. Studio"
                    />
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="share-years">Years made</FieldLabel>
                    <Input
                      id="share-years"
                      value={draft.years}
                      onChange={(e) => set("years")(e.target.value)}
                      placeholder="e.g. 2008–2015"
                    />
                  </Field>
                </div>
                <Field>
                  <FieldLabel htmlFor="share-variant">Version</FieldLabel>
                  <Input
                    id="share-variant"
                    value={draft.variant}
                    onChange={(e) => set("variant")(e.target.value)}
                    placeholder="e.g. built 2010 onwards"
                  />
                  <FieldDescription>
                    Only if this model was made in more than one form.
                  </FieldDescription>
                </Field>
                <Field>
                  <FieldLabel htmlFor="share-notes">
                    Before you start
                  </FieldLabel>
                  <Textarea
                    id="share-notes"
                    value={draft.notes}
                    onChange={(e) => set("notes")(e.target.value)}
                    placeholder={
                      "e.g. Set MIDI to mode B\nRecord from the MIDI SEQ socket"
                    }
                  />
                  <FieldDescription>
                    Anything another church should do on the organ first, one
                    per line.
                  </FieldDescription>
                </Field>
                <Field>
                  <FieldLabel htmlFor="share-note">
                    Note to the reviewer
                  </FieldLabel>
                  <Textarea
                    id="share-note"
                    value={draft.note}
                    onChange={(e) => set("note")(e.target.value)}
                    placeholder="Optional"
                  />
                </Field>
              </FieldGroup>
            </AccountGate>
          )}

          {problem && <ShareProblem message={problem} />}

          <DialogFooter>
            {result ? (
              <Button type="button" onClick={() => onOpenChange(false)}>
                Done
              </Button>
            ) : (
              <>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => onOpenChange(false)}
                >
                  Cancel
                </Button>
                {account?.signed_in && (
                  <Button type="submit" disabled={busy || !ready}>
                    {busy ? (
                      <Spinner data-icon="inline-start" />
                    ) : (
                      <SendIcon data-icon="inline-start" />
                    )}
                    Share
                  </Button>
                )}
              </>
            )}
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
