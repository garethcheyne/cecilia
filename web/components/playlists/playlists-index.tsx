"use client"

import * as React from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import { ListMusicIcon, PlusIcon } from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { PageBody } from "@/components/page-body"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { api, call, type PlaylistSummary } from "@/lib/api/client"
import { formatDate } from "@/lib/time"

function suggestedName() {
  // Next Sunday, e.g. "Sunday 16 November".
  const d = new Date()
  d.setDate(d.getDate() + ((7 - d.getDay()) % 7 || 7))
  return `Sunday ${d.toLocaleDateString(undefined, { day: "numeric", month: "long" })}`
}

export function NewPlaylistDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
}) {
  const router = useRouter()
  const [name, setName] = React.useState("")
  const [busy, setBusy] = React.useState(false)
  const [wasOpen, setWasOpen] = React.useState(false)
  if (open && !wasOpen) {
    setWasOpen(true)
    setName(suggestedName())
  }
  if (!open && wasOpen) setWasOpen(false)

  async function create(e: React.FormEvent) {
    e.preventDefault()
    if (!name.trim()) return
    setBusy(true)
    try {
      const p = await call(
        api.POST("/api/v1/playlists", { body: { name: name.trim() } })
      )
      onOpenChange(false)
      router.push(`/playlists/edit/?id=${encodeURIComponent(p.id)}`)
    } catch (err) {
      toast.error((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <form onSubmit={create} className="flex flex-col gap-6">
          <DialogHeader>
            <DialogTitle>New playlist</DialogTitle>
            <DialogDescription>
              An order of service: the music to play, in order.
            </DialogDescription>
          </DialogHeader>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="playlist-name">Name</FieldLabel>
              <Input
                id="playlist-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                autoFocus
              />
            </Field>
          </FieldGroup>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={busy || !name.trim()}>
              Create playlist
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export function PlaylistsIndex() {
  const { connected, service } = useEngine()
  const [list, setList] = React.useState<PlaylistSummary[] | null>(null)
  const [creating, setCreating] = React.useState(false)

  React.useEffect(() => {
    if (!connected) return
    call(api.GET("/api/v1/playlists"))
      .then(setList)
      .catch((e: Error) => toast.error(e.message))
  }, [connected])

  return (
    <PageBody>
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-muted-foreground">
          Plan each service here, then run it from the Service screen.
        </p>
        <Button onClick={() => setCreating(true)}>
          <PlusIcon data-icon="inline-start" />
          New playlist
        </Button>
      </div>
      {list === null ? (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 3 }, (_, i) => (
            <Skeleton key={i} className="h-24" />
          ))}
        </div>
      ) : list.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ListMusicIcon />
            </EmptyMedia>
            <EmptyTitle>No playlists yet</EmptyTitle>
            <EmptyDescription>
              Make one for Sunday: the voluntary, the hymns, and anything else
              to play.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button onClick={() => setCreating(true)}>
              <PlusIcon data-icon="inline-start" />
              New playlist
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {list.map((p) => (
            <Link
              key={p.id}
              href={`/playlists/edit/?id=${encodeURIComponent(p.id)}`}
              className="rounded-xl outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
            >
              <Card className="h-full transition-colors hover:bg-accent/40">
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <span className="truncate">{p.name}</span>
                    {service?.playlist_id === p.id && <Badge>Running</Badge>}
                  </CardTitle>
                  <CardDescription>
                    {p.tracks === 1 ? "1 piece" : `${p.tracks} pieces`} ·{" "}
                    {formatDate(p.modified)}
                  </CardDescription>
                </CardHeader>
              </Card>
            </Link>
          ))}
        </div>
      )}
      <NewPlaylistDialog open={creating} onOpenChange={setCreating} />
    </PageBody>
  )
}
