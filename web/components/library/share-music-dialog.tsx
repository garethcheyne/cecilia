"use client"

import * as React from "react"
import { SendIcon } from "lucide-react"

import {
  AccountGate,
  ShareDone,
  ShareProblem,
  useCommunityAccount,
} from "@/components/library/share-common"
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
} from "@/components/ui/field"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import {
  api,
  call,
  type Organist,
  type ShareResult,
  type TrackSummary,
} from "@/lib/api/client"

type Licence = "public-domain" | "CC0-1.0" | "CC-BY-4.0" | "CC-BY-SA-4.0"

const LICENCES: { value: Licence; label: string }[] = [
  {
    value: "public-domain",
    label: "Public domain (old music, e.g. hymns before 1900)",
  },
  { value: "CC0-1.0", label: "Free for anyone (CC0)" },
  { value: "CC-BY-4.0", label: "Credit needed (CC BY)" },
  { value: "CC-BY-SA-4.0", label: "Credit and share alike (CC BY-SA)" },
]

/** How the organist will be named: only with their permission. */
function creditFor(performer: string, organists: Organist[] | null): string {
  const p = performer.trim().toLowerCase()
  const o = p
    ? organists?.find((x) => x.name.trim().toLowerCase() === p)
    : undefined
  if (!o || !o.credit_publicly) return "A Cecilia organist"
  return (o.credit_as ?? "").trim() || o.name
}

/** Share a piece from this library with other churches. */
export function ShareMusicDialog({
  track,
  open,
  onOpenChange,
}: {
  track: TrackSummary | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { account, problem: accountProblem } = useCommunityAccount(open)
  const [organists, setOrganists] = React.useState<Organist[] | null>(null)
  const [licence, setLicence] = React.useState<Licence | null>(null)
  const [sure, setSure] = React.useState(false)
  const [note, setNote] = React.useState("")
  const [busy, setBusy] = React.useState(false)
  const [problem, setProblem] = React.useState<string | null>(null)
  const [result, setResult] = React.useState<ShareResult | null>(null)

  // Start afresh each time it opens.
  const [wasOpen, setWasOpen] = React.useState(open)
  if (open !== wasOpen) {
    setWasOpen(open)
    if (open) {
      setLicence(null)
      setSure(false)
      setNote("")
      setProblem(null)
      setResult(null)
    }
  }

  React.useEffect(() => {
    if (!open) return
    call(api.GET("/api/v1/organists"))
      .then((b) => setOrganists(b.organists))
      .catch(() => setOrganists([]))
  }, [open])

  const credit = creditFor(track?.info.performer ?? "", organists)
  const ready = !!track && !!licence && sure

  async function send(e: React.FormEvent) {
    e.preventDefault()
    if (!track || !licence || !sure) return
    setBusy(true)
    setProblem(null)
    try {
      const r = await call(
        api.POST("/api/v1/community/share/music/{id}", {
          params: { path: { id: track.id } },
          body: { licence, note: note.trim() },
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
              {track?.info.title}. Only music that&apos;s public domain or
              openly licensed may be shared. Modern hymns and worship songs are
              usually still in copyright, even if your church has a CCLI
              licence.
            </DialogDescription>
          </DialogHeader>

          {result ? (
            <ShareDone result={result} />
          ) : (
            <AccountGate account={account} problem={accountProblem}>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="share-licence">
                    Why it&apos;s free to share
                  </FieldLabel>
                  <Select
                    value={licence}
                    onValueChange={(v) => setLicence((v as Licence) ?? null)}
                    items={LICENCES}
                  >
                    <SelectTrigger id="share-licence" className="w-full">
                      <SelectValue placeholder="Choose one" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        {LICENCES.map((l) => (
                          <SelectItem key={l.value} value={l.value}>
                            {l.label}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                  <FieldDescription>
                    The tune and its harmony both need to be free: their
                    composers died more than 70 years ago, or they were written
                    for anyone to use.
                  </FieldDescription>
                </Field>
                <Field orientation="horizontal">
                  <Checkbox
                    id="share-sure"
                    checked={sure}
                    onCheckedChange={(v) => setSure(v === true)}
                  />
                  <FieldLabel htmlFor="share-sure" className="font-normal">
                    I&apos;m sure this music is free to share
                  </FieldLabel>
                </Field>
                <Field>
                  <FieldLabel>Credited as</FieldLabel>
                  <p className="text-sm">{credit}</p>
                  <FieldDescription>
                    The organist is named only if their profile in Settings says
                    they&apos;re happy to be credited publicly.
                  </FieldDescription>
                </Field>
                <Field>
                  <FieldLabel htmlFor="share-music-note">
                    Note to the reviewer
                  </FieldLabel>
                  <Textarea
                    id="share-music-note"
                    value={note}
                    onChange={(e) => setNote(e.target.value)}
                    placeholder="Optional, e.g. where the music comes from"
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
