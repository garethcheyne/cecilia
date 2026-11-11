"use client"

import * as React from "react"
import Link from "next/link"
import { useRouter, useSearchParams } from "next/navigation"
import {
  ArrowLeftIcon,
  CopyIcon,
  EarIcon,
  PencilIcon,
  PlusIcon,
  SaveIcon,
  Share2Icon,
  Trash2Icon,
  WandSparklesIcon,
} from "lucide-react"
import { toast } from "sonner"

import { CONSOLE_ORDER } from "@/components/organ/divisions"
import { LearnDialog } from "@/components/organs/learn-dialog"
import { uniqueName } from "@/components/organs/organ-names"
import { ShareOrganDialog } from "@/components/organs/share-organ-dialog"
import { MANUFACTURERS } from "@/components/organs/organs-index"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
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
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { useMidiCapture } from "@/hooks/use-midi-capture"
import {
  api,
  call,
  type DivisionRole,
  type OrganDefinition,
  type Stop,
} from "@/lib/api/client"
import {
  describeBinding,
  firstControl,
  hex,
  keyboard,
  valueBinding,
  type StopBinding,
} from "@/lib/learn"
import { noteName } from "@/lib/midi"
import { divisionName, stopLabel } from "@/lib/stops"

type Equivalent = Stop["equivalent"]
const FAMILIES = [
  "principal",
  "flute",
  "string",
  "celeste",
  "reed",
  "mutation",
  "mixture",
  "percussion",
  "other",
] as const
const ROLES: DivisionRole[] = [
  "pedal",
  "great",
  "swell",
  "choir",
  "solo",
  "echo",
]

function opts(
  values: readonly string[],
  label: (v: string) => string = (v) => v[0].toUpperCase() + v.slice(1)
) {
  return values.map((v) => ({ value: v, label: label(v) }))
}

function SimpleSelect({
  value,
  items,
  onChange,
  label,
  className,
  disabled,
}: {
  value: string
  items: { value: string; label: string }[]
  onChange: (v: string) => void
  label: string
  className?: string
  disabled?: boolean
}) {
  return (
    <Select
      value={value}
      items={items}
      disabled={disabled}
      onValueChange={(v) => v != null && onChange(String(v))}
    >
      <SelectTrigger className={className ?? "w-full"} aria-label={label}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          {items.map((i) => (
            <SelectItem key={i.value} value={i.value}>
              {i.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  )
}

/** Listen for something at the console, then hand back what was heard. */
function ListenButton({
  label,
  prompt,
  onHeard,
  ready,
}: {
  label: string
  prompt: string
  /** Decide from the messages so far; return true when done. */
  onHeard: (msgs: number[][]) => boolean
  ready?: (msgs: number[][]) => boolean
}) {
  const [active, setActive] = React.useState(false)
  const msgs = useMidiCapture(active)
  const canFinish = ready ? ready(msgs) : msgs.length > 0
  return active ? (
    <div className="flex flex-wrap items-center gap-2 rounded-lg bg-primary/5 px-3 py-2 text-sm ring-1 ring-primary/20">
      <Spinner />
      <span className="flex-1">{prompt}</span>
      <Button
        size="sm"
        disabled={!canFinish}
        onClick={() => {
          if (onHeard(msgs)) setActive(false)
          else toast.error("Nothing usable was heard. Try again.")
        }}
      >
        Done
      </Button>
      <Button size="sm" variant="ghost" onClick={() => setActive(false)}>
        Cancel
      </Button>
    </div>
  ) : (
    <Button variant="outline" size="sm" onClick={() => setActive(true)}>
      <EarIcon data-icon="inline-start" />
      {label}
    </Button>
  )
}

function StopDialog({
  organ,
  stop,
  onSave,
  onOpenChange,
}: {
  organ: OrganDefinition
  stop: Stop | null
  onSave: (s: Stop) => void
  onOpenChange: (o: boolean) => void
}) {
  const [draft, setDraft] = React.useState<Stop | null>(null)
  const [shownFor, setShownFor] = React.useState<string | null>(null)
  if (stop && shownFor !== stop.id) {
    setShownFor(stop.id)
    setDraft(structuredClone(stop))
  }
  if (!stop && shownFor) setShownFor(null)
  if (!draft) return null
  const eq = draft.equivalent
  const setEq = (e: Equivalent) => setDraft({ ...draft, equivalent: e })
  const roles = organ.divisions.map((d) => d.role)
  const binding = draft.midi?.[0]
  const setBinding = (b: StopBinding | null) =>
    setDraft({ ...draft, midi: b ? [b, ...(draft.midi ?? []).slice(1)] : [] })

  return (
    <Dialog open={!!stop} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Stop</DialogTitle>
          <DialogDescription>
            What it&apos;s called, what it is, and its MIDI.
          </DialogDescription>
        </DialogHeader>
        <FieldGroup className="max-h-[60svh] overflow-y-auto px-0.5">
          <Field>
            <FieldLabel>Name on the stop (up to three lines)</FieldLabel>
            <div className="grid grid-cols-3 gap-2">
              {[0, 1, 2].map((i) => (
                <Input
                  key={i}
                  value={draft.names[i] ?? ""}
                  aria-label={`Line ${i + 1}`}
                  onChange={(e) => {
                    const names = [...draft.names]
                    names[i] = e.target.value
                    setDraft({
                      ...draft,
                      names: names
                        .map((n) => n ?? "")
                        .filter((n, j) => n !== "" || j < i),
                    })
                  }}
                />
              ))}
            </div>
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field>
              <FieldLabel>Keyboard</FieldLabel>
              <SimpleSelect
                label="Keyboard"
                value={draft.division}
                items={roles.map((r) => ({
                  value: r,
                  label: divisionName(r, organ),
                }))}
                onChange={(v) =>
                  setDraft({ ...draft, division: v as DivisionRole })
                }
              />
            </Field>
            <Field>
              <FieldLabel>Kind</FieldLabel>
              <SimpleSelect
                label="Kind"
                value={eq.type}
                items={[
                  { value: "voice", label: "Speaking stop" },
                  { value: "coupler", label: "Coupler" },
                  { value: "tremulant", label: "Tremulant" },
                  { value: "unison_off", label: "Unison off" },
                ]}
                onChange={(v) =>
                  setEq(
                    v === "voice"
                      ? { type: "voice", family: "principal", footage: "8" }
                      : v === "coupler"
                        ? {
                            type: "coupler",
                            source:
                              roles.find((r) => r !== draft.division) ??
                              "swell",
                            pitch: "unison",
                          }
                        : ({ type: v } as Equivalent)
                  )
                }
              />
            </Field>
          </div>
          {eq.type === "voice" && (
            <div className="grid grid-cols-2 gap-3">
              <Field>
                <FieldLabel>Family</FieldLabel>
                <SimpleSelect
                  label="Family"
                  value={eq.family}
                  items={opts(FAMILIES)}
                  onChange={(v) =>
                    setEq({ ...eq, family: v as (typeof FAMILIES)[number] })
                  }
                />
              </Field>
              <Field>
                <FieldLabel>Pitch (feet)</FieldLabel>
                <Input
                  value={eq.footage}
                  onChange={(e) => setEq({ ...eq, footage: e.target.value })}
                  placeholder='8, 4, "2 2/3"'
                />
              </Field>
            </div>
          )}
          {eq.type === "coupler" && (
            <div className="grid grid-cols-2 gap-3">
              <Field>
                <FieldLabel>Couples from</FieldLabel>
                <SimpleSelect
                  label="Couples from"
                  value={eq.source}
                  items={roles.map((r) => ({
                    value: r,
                    label: divisionName(r, organ),
                  }))}
                  onChange={(v) => setEq({ ...eq, source: v as DivisionRole })}
                />
              </Field>
              <Field>
                <FieldLabel>Pitch</FieldLabel>
                <SimpleSelect
                  label="Coupler pitch"
                  value={eq.pitch}
                  items={[
                    { value: "unison", label: "Unison" },
                    { value: "sub", label: "Octave down (16′)" },
                    { value: "super", label: "Octave up (4′)" },
                  ]}
                  onChange={(v) =>
                    setEq({ ...eq, pitch: v as "unison" | "sub" | "super" })
                  }
                />
              </Field>
            </div>
          )}
          <Field>
            <FieldLabel>Shape</FieldLabel>
            <SimpleSelect
              label="Shape"
              value={draft.style?.shape ?? "round"}
              items={[
                { value: "round", label: "Drawknob (round)" },
                { value: "rectangle", label: "Tab (rectangle)" },
              ]}
              onChange={(v) =>
                setDraft({
                  ...draft,
                  style: { ...draft.style, shape: v as "round" | "rectangle" },
                })
              }
            />
          </Field>
          <Field>
            <FieldLabel>MIDI</FieldLabel>
            <div className="grid grid-cols-[8rem_1fr] gap-2">
              <SimpleSelect
                label="MIDI type"
                value={binding?.type ?? "none"}
                items={[
                  { value: "none", label: "None" },
                  { value: "toggle", label: "Toggle" },
                  { value: "on_off", label: "On / off" },
                  { value: "switch", label: "Value switch" },
                ]}
                onChange={(v) =>
                  setBinding(
                    v === "toggle"
                      ? { type: "toggle", msg: "C0 00" }
                      : v === "on_off"
                        ? { type: "on_off", on: "B0 50 7F", off: "B0 50 00" }
                        : v === "switch"
                          ? {
                              type: "switch",
                              pattern: "B0 50 vv",
                              on: 127,
                              off: 0,
                            }
                          : null
                  )
                }
              />
              {binding?.type === "toggle" && (
                <Input
                  className="font-mono"
                  value={binding.msg}
                  onChange={(e) =>
                    setBinding({ ...binding, msg: e.target.value })
                  }
                  aria-label="Toggle message"
                />
              )}
              {binding?.type === "on_off" && (
                <div className="grid grid-cols-2 gap-2">
                  <Input
                    className="font-mono"
                    value={binding.on}
                    onChange={(e) =>
                      setBinding({ ...binding, on: e.target.value })
                    }
                    aria-label="On message"
                  />
                  <Input
                    className="font-mono"
                    value={binding.off}
                    onChange={(e) =>
                      setBinding({ ...binding, off: e.target.value })
                    }
                    aria-label="Off message"
                  />
                </div>
              )}
              {binding?.type === "switch" && (
                <Input
                  className="font-mono"
                  value={binding.pattern}
                  onChange={(e) =>
                    setBinding({ ...binding, pattern: e.target.value })
                  }
                  aria-label="Switch pattern"
                />
              )}
            </div>
            <FieldDescription>
              Hex bytes, e.g. “CB 12”. Use Learn on the stop list to fill this
              in from the organ.
            </FieldDescription>
          </Field>
        </FieldGroup>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            onClick={() => {
              onSave({
                ...draft,
                names: draft.names.filter((n) => n.trim() !== ""),
              })
              onOpenChange(false)
            }}
          >
            Done
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function OrganEditor() {
  const router = useRouter()
  const id = useSearchParams().get("id") ?? ""
  const [organ, setOrgan] = React.useState<OrganDefinition | null>(null)
  const [builtin, setBuiltin] = React.useState(false)
  const [names, setNames] = React.useState<string[]>([])
  const [copying, setCopying] = React.useState(false)
  const [missing, setMissing] = React.useState(false)
  const [dirty, setDirty] = React.useState(false)
  const [sharing, setSharing] = React.useState(false)
  const [saving, setSaving] = React.useState(false)
  const [problem, setProblem] = React.useState<string | null>(null)
  const [editing, setEditing] = React.useState<Stop | null>(null)
  const [learning, setLearning] = React.useState<string[] | null>(null)

  React.useEffect(() => {
    if (!id) return
    Promise.all([
      call(api.GET("/api/v1/organs/{id}", { params: { path: { id } } })),
      call(api.GET("/api/v1/organs")),
    ])
      .then(([o, list]) => {
        setOrgan(o)
        setBuiltin(!!list.find((x) => x.id === id)?.builtin)
        setNames(list.map((x) => x.name))
      })
      .catch(() => setMissing(true))
  }, [id])

  const update = (o: OrganDefinition) => {
    setOrgan(o)
    setDirty(true)
    setProblem(null)
  }
  const onLearned = React.useCallback((stopId: string, b: StopBinding) => {
    setOrgan((o) =>
      o
        ? {
            ...o,
            stops: o.stops.map((s) =>
              s.id === stopId ? { ...s, midi: [b] } : s
            ),
          }
        : o
    )
    setDirty(true)
  }, [])

  /** A built-in organ can't change: copy it and carry on in the copy. */
  async function makeCopy() {
    if (!organ) return
    setCopying(true)
    try {
      const copy = await call(
        api.POST("/api/v1/organs/copy", {
          body: { from: organ.id, name: uniqueName("Our organ", names) },
        })
      )
      toast.success(`Made ${copy.name}. Teach it your organ's stops here.`)
      router.push(`/organs/edit/?id=${encodeURIComponent(copy.id)}`)
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      setCopying(false)
    }
  }

  async function save() {
    if (!organ) return
    setSaving(true)
    try {
      const saved = await call(
        api.PUT("/api/v1/organs/{id}/save", {
          params: { path: { id: organ.id } },
          body: organ,
        })
      )
      setOrgan(saved)
      setDirty(false)
      toast.success("Organ saved")
    } catch (e) {
      setProblem((e as Error).message)
    } finally {
      setSaving(false)
    }
  }

  if (!id || missing) {
    return (
      <div className="p-4">
        <Empty className="border">
          <EmptyHeader>
            <EmptyTitle>Organ not found</EmptyTitle>
            <EmptyDescription>
              <Link href="/organs">Back to organs</Link>
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      </div>
    )
  }
  if (!organ) return <Skeleton className="m-4 h-96" />

  // Top keyboard first, pedals last, as at the console.
  const divisions = [...organ.divisions].sort(
    (a, b) => CONSOLE_ORDER.indexOf(a.role) - CONSOLE_ORDER.indexOf(b.role)
  )

  const setDivision = (
    role: DivisionRole,
    patch: Partial<OrganDefinition["divisions"][number]>
  ) =>
    update({
      ...organ,
      divisions: organ.divisions.map((d) =>
        d.role === role ? { ...d, ...patch } : d
      ),
    })

  const addStop = (role: DivisionRole) => {
    const here = organ.stops.filter((s) => s.division === role)
    const base = here[here.length - 1]
    let n = here.length + 1
    while (organ.stops.some((s) => s.id === `${role}-stop-${n}`)) n++
    const stop: Stop = {
      ...(base
        ? structuredClone(base)
        : {
            division: role,
            equivalent: { type: "voice", family: "flute", footage: "8" },
            names: [],
          }),
      id: `${role}-stop-${n}`,
      names: ["New", "Stop"],
      midi: [],
      equivalent: {
        type: "voice",
        family: "other",
        footage: "8",
        tag: `New ${n}`,
      },
    } as Stop
    update({ ...organ, stops: [...organ.stops, stop] })
    setEditing(stop)
  }

  return (
    <div className="mx-auto flex w-full max-w-6xl min-w-0 flex-col gap-4 p-3 sm:p-4">
      <div className="flex flex-wrap items-center gap-2">
        <Button
          variant="ghost"
          size="icon"
          render={<Link href="/organs" />}
          nativeButton={false}
          aria-label="Back to organs"
        >
          <ArrowLeftIcon />
        </Button>
        <h2 className="min-w-0 flex-1 truncate font-display text-xl font-semibold">
          {organ.name}
        </h2>
        {builtin ? (
          <Badge variant="secondary">Built in</Badge>
        ) : (
          <>
            <span className="text-sm text-muted-foreground" aria-live="polite">
              {saving ? "Saving…" : dirty ? "Unsaved changes" : "Saved"}
            </span>
            <Button
              variant="outline"
              onClick={() => setSharing(true)}
              disabled={dirty || saving}
              title={
                dirty
                  ? "Save your changes first"
                  : "Share this setup with other churches"
              }
            >
              <Share2Icon data-icon="inline-start" />
              <span className="hidden sm:inline">
                Share with other churches
              </span>
              <span className="sm:hidden">Share</span>
            </Button>
            <Button onClick={save} disabled={!dirty || saving}>
              <SaveIcon data-icon="inline-start" />
              Save
            </Button>
          </>
        )}
      </div>
      {!builtin && (
        <ShareOrganDialog
          organ={organ}
          open={sharing}
          onOpenChange={setSharing}
        />
      )}
      {builtin && (
        <Alert>
          <CopyIcon />
          <AlertTitle>This organ is built in</AlertTitle>
          <AlertDescription className="flex flex-col items-start gap-3">
            <span>
              To set up your own organ from it, make a copy. You can teach the
              copy your organ&apos;s stops.
            </span>
            <Button size="sm" onClick={makeCopy} disabled={copying}>
              {copying ? (
                <Spinner data-icon="inline-start" />
              ) : (
                <CopyIcon data-icon="inline-start" />
              )}
              Make my own copy
            </Button>
          </AlertDescription>
        </Alert>
      )}
      {problem && (
        <Alert variant="destructive">
          <AlertTitle>Can&apos;t save yet</AlertTitle>
          <AlertDescription>{problem}</AlertDescription>
        </Alert>
      )}

      <Tabs defaultValue="stops">
        <TabsList>
          <TabsTrigger value="stops">Stops</TabsTrigger>
          <TabsTrigger value="keyboards">Keyboards and pedals</TabsTrigger>
          <TabsTrigger value="about">About</TabsTrigger>
        </TabsList>

        <TabsContent value="stops" className="flex flex-col gap-4 pt-2">
          {!builtin && (
            <div className="flex flex-wrap items-center gap-3">
              <Button onClick={() => setLearning(organ.stops.map((s) => s.id))}>
                <WandSparklesIcon data-icon="inline-start" />
                Learn every stop
              </Button>
              <p className="text-sm text-muted-foreground">
                Pull out and push in each stop at the console when asked. It
                takes a few minutes.
              </p>
            </div>
          )}
          {divisions.map((d) => (
            <Card key={d.role}>
              <CardHeader>
                <CardTitle>{d.name}</CardTitle>
                <CardDescription>
                  {organ.stops.filter((s) => s.division === d.role).length}{" "}
                  stops
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-1">
                {organ.stops
                  .filter((s) => s.division === d.role)
                  .map((s) => (
                    <div
                      key={s.id}
                      className="flex flex-wrap items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-muted/50"
                    >
                      <span className="min-w-40 flex-1 font-medium">
                        {stopLabel(
                          { division: s.division, equivalent: s.equivalent },
                          organ
                        )}
                      </span>
                      {/* The MIDI message itself is for whoever sets it up: on hover, and in the stop's edit dialog. */}
                      <Badge
                        variant={s.midi?.length ? "secondary" : "outline"}
                        title={describeBinding(s.midi?.[0])}
                      >
                        {s.midi?.length ? "Learned" : "Not learned"}
                      </Badge>
                      {!builtin && (
                        <div className="flex">
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setLearning([s.id])}
                          >
                            <EarIcon data-icon="inline-start" />
                            Learn
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            onClick={() => setEditing(s)}
                            aria-label={`Edit ${s.names.join(" ")}`}
                          >
                            <PencilIcon />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            onClick={() =>
                              update({
                                ...organ,
                                stops: organ.stops.filter((x) => x.id !== s.id),
                              })
                            }
                            aria-label={`Delete ${s.names.join(" ")}`}
                          >
                            <Trash2Icon />
                          </Button>
                        </div>
                      )}
                    </div>
                  ))}
                {!builtin && (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="self-start"
                    onClick={() => addStop(d.role)}
                  >
                    <PlusIcon data-icon="inline-start" />
                    Add a stop
                  </Button>
                )}
              </CardContent>
            </Card>
          ))}
        </TabsContent>

        <TabsContent value="keyboards" className="flex flex-col gap-4 pt-2">
          {divisions.map((d) => (
            <Card key={d.role}>
              <CardHeader>
                <CardTitle>{d.name}</CardTitle>
                <CardDescription>
                  Channel {d.channel} · {noteName(d.low_key)} to{" "}
                  {noteName(d.high_key)}
                  {d.expression ? ` · swell pedal ${d.expression.pattern}` : ""}
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                  <Field>
                    <FieldLabel>Name</FieldLabel>
                    <Input
                      value={d.name}
                      disabled={builtin}
                      onChange={(e) =>
                        setDivision(d.role, { name: e.target.value })
                      }
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Role</FieldLabel>
                    <SimpleSelect
                      label="Role"
                      value={d.role}
                      items={opts(ROLES)}
                      onChange={() => {}}
                      className="w-full"
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Lowest key</FieldLabel>
                    <Input
                      type="number"
                      min={0}
                      max={127}
                      value={d.low_key}
                      disabled={builtin}
                      onChange={(e) =>
                        setDivision(d.role, { low_key: Number(e.target.value) })
                      }
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Highest key</FieldLabel>
                    <Input
                      type="number"
                      min={0}
                      max={127}
                      value={d.high_key}
                      disabled={builtin}
                      onChange={(e) =>
                        setDivision(d.role, {
                          high_key: Number(e.target.value),
                        })
                      }
                    />
                  </Field>
                </div>
                {!builtin && (
                  <div className="flex flex-col gap-2">
                    <ListenButton
                      label="Learn the keys"
                      prompt={`Play the lowest and then the highest key on the ${d.name}, then press Done.`}
                      ready={(m) => !!keyboard(m)}
                      onHeard={(m) => {
                        const k = keyboard(m)
                        if (!k) return false
                        setDivision(d.role, {
                          channel: k.channel,
                          low_key: k.low,
                          high_key: k.high,
                        })
                        toast.success(
                          `${d.name}: channel ${k.channel}, ${noteName(k.low)} to ${noteName(k.high)}`
                        )
                        return true
                      }}
                    />
                    <ListenButton
                      label={
                        d.expression
                          ? "Relearn the swell pedal"
                          : "Learn a swell pedal"
                      }
                      prompt={`Move the ${d.name} swell pedal fully shut and fully open, then press Done.`}
                      ready={(m) => !!valueBinding(m)}
                      onHeard={(m) => {
                        const v = valueBinding(m)
                        if (!v) return false
                        setDivision(d.role, { expression: v })
                        toast.success(
                          `${d.name} swell pedal: ${v.pattern}, ${v.min}–${v.max}`
                        )
                        return true
                      }}
                    />
                  </div>
                )}
              </CardContent>
            </Card>
          ))}
          <Card>
            <CardHeader>
              <CardTitle>General cancel</CardTitle>
              <CardDescription>
                {organ.all_stops_off
                  ? `Sends ${organ.all_stops_off}`
                  : "None set: Cecilia pushes stops in one by one. Organs whose stops only toggle need this."}
              </CardDescription>
            </CardHeader>
            {!builtin && (
              <CardContent>
                <ListenButton
                  label="Learn the general cancel"
                  prompt="Press the General Cancel piston, then press Done."
                  ready={(m) => !!firstControl(m)}
                  onHeard={(m) => {
                    const c = firstControl(m)
                    if (!c) return false
                    update({ ...organ, all_stops_off: hex(c) })
                    return true
                  }}
                />
              </CardContent>
            )}
          </Card>
        </TabsContent>

        <TabsContent value="about" className="flex flex-col gap-4 pt-2">
          <Card>
            <CardContent>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="organ-name">Name</FieldLabel>
                  <Input
                    id="organ-name"
                    value={organ.name}
                    disabled={builtin}
                    onChange={(e) => update({ ...organ, name: e.target.value })}
                  />
                </Field>
                <Field>
                  <FieldLabel>Maker</FieldLabel>
                  <SimpleSelect
                    label="Maker"
                    disabled={builtin}
                    value={organ.manufacturer}
                    items={Object.entries(MANUFACTURERS).map(
                      ([value, label]) => ({ value, label })
                    )}
                    onChange={(v) =>
                      update({
                        ...organ,
                        manufacturer: v as OrganDefinition["manufacturer"],
                      })
                    }
                  />
                </Field>
                <div className="grid gap-3 sm:grid-cols-2">
                  <Field>
                    <FieldLabel>Record from socket</FieldLabel>
                    <Input
                      value={organ.connection?.record_from ?? ""}
                      disabled={builtin}
                      placeholder="e.g. MIDI SEQ"
                      onChange={(e) =>
                        update({
                          ...organ,
                          connection: {
                            ...organ.connection,
                            record_from: e.target.value || null,
                          },
                        })
                      }
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Play back into socket</FieldLabel>
                    <Input
                      value={organ.connection?.play_to ?? ""}
                      disabled={builtin}
                      placeholder="e.g. MIDI IN"
                      onChange={(e) =>
                        update({
                          ...organ,
                          connection: {
                            ...organ.connection,
                            play_to: e.target.value || null,
                          },
                        })
                      }
                    />
                  </Field>
                </div>
                <Field>
                  <FieldLabel>Notes for whoever sets it up</FieldLabel>
                  <Textarea
                    rows={4}
                    disabled={builtin}
                    value={(organ.connection?.notes ?? []).join("\n")}
                    onChange={(e) =>
                      update({
                        ...organ,
                        connection: {
                          ...organ.connection,
                          notes: e.target.value.split("\n").filter(Boolean),
                        },
                      })
                    }
                  />
                </Field>
              </FieldGroup>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      <StopDialog
        organ={organ}
        stop={editing}
        onOpenChange={(o) => !o && setEditing(null)}
        onSave={(s) =>
          update({
            ...organ,
            stops: organ.stops.map((x) => (x.id === s.id ? s : x)),
          })
        }
      />
      {learning && (
        <LearnDialog
          organ={organ}
          stopIds={learning}
          open={!!learning}
          onOpenChange={(o) => !o && setLearning(null)}
          onLearned={onLearned}
        />
      )}
    </div>
  )
}
