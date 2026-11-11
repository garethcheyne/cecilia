"use client"

import * as React from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import { CheckIcon, CopyIcon, PencilIcon, Trash2Icon } from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardAction, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { api, call, type OrganSummary } from "@/lib/api/client"

export const MANUFACTURERS: Record<string, string> = {
  generic: "Generic",
  hauptwerk: "Hauptwerk",
  makin: "Makin",
  johannus: "Johannus",
  johannus_programmable: "Johannus (programmable)",
  allen: "Allen",
  rodgers: "Rodgers",
  rodgers_phase2: "Rodgers (Phase II)",
  viscount: "Viscount",
  peterson: "Peterson",
  solid_state1: "Solid State (type 1)",
  solid_state2: "Solid State (type 2)",
  walker: "Walker",
  wyvern: "Wyvern",
  content: "Content",
  classic_organ_works: "Classic Organ Works",
}

export function OrgansIndex() {
  const router = useRouter()
  const { connected, simulator } = useEngine()
  const [organs, setOrgans] = React.useState<OrganSummary[] | null>(null)
  const [copying, setCopying] = React.useState<OrganSummary | null>(null)
  const [name, setName] = React.useState("")
  const [deleting, setDeleting] = React.useState<OrganSummary | null>(null)
  const [version, setVersion] = React.useState(0)

  React.useEffect(() => {
    if (!connected) return
    call(api.GET("/api/v1/organs")).then(setOrgans).catch((e: Error) => toast.error(e.message))
  }, [connected, version])

  async function use(o: OrganSummary) {
    try {
      await call(api.PUT("/api/v1/simulator/organ", { body: { organ_id: o.id } }))
      toast.success(`Now using ${o.name}`)
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  async function copy(e: React.FormEvent) {
    e.preventDefault()
    if (!copying || !name.trim()) return
    try {
      const o = await call(api.POST("/api/v1/organs/copy", { body: { from: copying.id, name: name.trim() } }))
      router.push(`/organs/edit/?id=${encodeURIComponent(o.id)}`)
    } catch (err) {
      toast.error((err as Error).message)
    }
  }

  async function remove(o: OrganSummary) {
    try {
      await call(api.POST("/api/v1/organs/{id}/delete", { params: { path: { id: o.id } } }))
      toast.success(`Deleted ${o.name}`)
      setVersion((v) => v + 1)
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  return (
    <div className="mx-auto flex w-full min-w-0 max-w-5xl flex-col gap-4 p-3 sm:p-4">
      <p className="text-sm text-muted-foreground">
        The organ in use decides how Cecilia talks MIDI. To set up your own instrument, copy the closest template (the
        Makin one for a Makin) and teach it your stops with MIDI learn.
      </p>
      {organs === null ? (
        <div className="grid gap-3 sm:grid-cols-2">
          <Skeleton className="h-40" />
          <Skeleton className="h-40" />
        </div>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2">
          {organs.map((o) => {
            const inUse = simulator?.organ_id === o.id
            return (
              <Card key={o.id}>
                <CardHeader>
                  <CardTitle>{o.name}</CardTitle>
                  <CardDescription>
                    {MANUFACTURERS[o.manufacturer] ?? o.manufacturer} · {o.divisions} keyboards · {o.stops} stops
                  </CardDescription>
                  <CardAction className="flex gap-1">
                    {inUse && <Badge>In use</Badge>}
                    {o.builtin && <Badge variant="secondary">Built in</Badge>}
                  </CardAction>
                </CardHeader>
                <CardFooter className="flex-wrap gap-2">
                  <Button size="sm" variant={inUse ? "secondary" : "default"} disabled={inUse} onClick={() => use(o)}>
                    <CheckIcon data-icon="inline-start" />
                    {inUse ? "In use" : "Use this organ"}
                  </Button>
                  {!o.builtin && (
                    <Button size="sm" variant="outline" render={<Link href={`/organs/edit/?id=${encodeURIComponent(o.id)}`} />} nativeButton={false}>
                      <PencilIcon data-icon="inline-start" />
                      Edit
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      setCopying(o)
                      setName(o.builtin ? "Our organ" : `${o.name} (copy)`)
                    }}
                  >
                    <CopyIcon data-icon="inline-start" />
                    {o.builtin ? "Copy to set up" : "Copy"}
                  </Button>
                  {!o.builtin && (
                    <Button size="sm" variant="ghost" onClick={() => setDeleting(o)} aria-label={`Delete ${o.name}`}>
                      <Trash2Icon />
                    </Button>
                  )}
                </CardFooter>
              </Card>
            )
          })}
        </div>
      )}

      <Dialog open={!!copying} onOpenChange={(o) => !o && setCopying(null)}>
        <DialogContent className="sm:max-w-md">
          <form onSubmit={copy} className="flex flex-col gap-6">
            <DialogHeader>
              <DialogTitle>Copy {copying?.name}</DialogTitle>
              <DialogDescription>Give your organ a name. You can change its stops and MIDI next.</DialogDescription>
            </DialogHeader>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="organ-name">Name</FieldLabel>
                <Input id="organ-name" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
              </Field>
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setCopying(null)}>
                Cancel
              </Button>
              <Button type="submit" disabled={!name.trim()}>
                Copy and edit
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <AlertDialog open={!!deleting} onOpenChange={(o) => !o && setDeleting(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {deleting?.name}?</AlertDialogTitle>
            <AlertDialogDescription>Recordings made on it keep playing on whichever organ is in use.</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Keep it</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => deleting && remove(deleting)}>
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
