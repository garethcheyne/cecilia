"use client"

import * as React from "react"
import { PlusIcon, Trash2Icon } from "lucide-react"
import { toast } from "sonner"

import { useOrganists } from "@/components/settings/use-organists"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
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
import { Switch } from "@/components/ui/switch"
import type { Organist, OrganistsBody } from "@/lib/api/client"

const NOBODY = "nobody"

/** The people who play here, who's playing now, and whether each may be named. */
export function OrganistsCard() {
  const { data, problem, save } = useOrganists()
  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>Organists</CardTitle>
        <CardDescription>
          New recordings are credited to whoever is playing. Names stay on this
          computer unless a recording is shared, and then only for organists
          who&apos;ve agreed.
        </CardDescription>
      </CardHeader>
      {problem ? (
        <CardContent>
          <p className="text-sm text-muted-foreground">{problem}</p>
        </CardContent>
      ) : !data ? (
        <CardContent>
          <Skeleton className="h-24" />
        </CardContent>
      ) : (
        // Start a fresh draft whenever the saved list changes.
        <OrganistsEditor
          key={JSON.stringify(data)}
          saved={data}
          onSave={save}
        />
      )}
    </Card>
  )
}

/** A draft organist; new ones have no id until saved. */
type Draft = Organist & { key: string }

let drafts = 0
const stripKey = (d: Draft): Organist => ({
  id: d.id,
  name: d.name,
  credit_as: d.credit_as,
  credit_publicly: d.credit_publicly,
})
const draftOf = (o: Organist): Draft => ({
  ...o,
  key: o.id || `new-${++drafts}`,
})

function OrganistsEditor({
  saved,
  onSave,
}: {
  saved: OrganistsBody
  onSave: (b: OrganistsBody) => Promise<OrganistsBody>
}) {
  const [list, setList] = React.useState<Draft[]>(() =>
    saved.organists.map(draftOf)
  )
  const [current, setCurrent] = React.useState<string | null>(
    saved.current ?? null
  )
  const [busy, setBusy] = React.useState(false)

  const changed =
    JSON.stringify(list.map(stripKey)) !== JSON.stringify(saved.organists) ||
    current !== (saved.current ?? null)
  const missingName = list.some((o) => o.name.trim() === "")

  const update = (key: string, patch: Partial<Organist>) =>
    setList((l) => l.map((o) => (o.key === key ? { ...o, ...patch } : o)))

  function add() {
    setList((l) => [
      ...l,
      draftOf({ id: "", name: "", credit_as: "", credit_publicly: false }),
    ])
  }

  function remove(key: string) {
    const o = list.find((x) => x.key === key)
    if (o && o.id && o.id === current) setCurrent(null)
    setList((l) => l.filter((x) => x.key !== key))
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    if (missingName) return
    setBusy(true)
    try {
      await onSave({ organists: list.map(stripKey), current })
      toast.success("Organists saved")
    } catch (err) {
      toast.error((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const choices = [
    { value: NOBODY, label: "Nobody chosen" },
    ...list
      .filter((o) => o.id && o.name.trim())
      .map((o) => ({ value: o.id, label: o.name })),
  ]

  return (
    <form onSubmit={submit}>
      <CardContent className="flex flex-col gap-6">
        {list.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No organists yet. Add the people who play here, so recordings say
            who played them.
          </p>
        ) : (
          <ul className="flex flex-col gap-4">
            {list.map((o, i) => (
              <li key={o.key} className="rounded-xl border p-4">
                <FieldGroup className="gap-4">
                  <div className="grid gap-4 sm:grid-cols-2">
                    <Field data-invalid={o.name.trim() === "" || undefined}>
                      <FieldLabel htmlFor={`organist-name-${o.key}`}>
                        Name
                      </FieldLabel>
                      <Input
                        id={`organist-name-${o.key}`}
                        value={o.name}
                        onChange={(e) =>
                          update(o.key, { name: e.target.value })
                        }
                        placeholder="e.g. Margaret Smith"
                        aria-invalid={o.name.trim() === "" || undefined}
                        autoFocus={i === list.length - 1 && !o.id}
                      />
                    </Field>
                    <Field>
                      <FieldLabel htmlFor={`organist-credit-${o.key}`}>
                        Credit as
                      </FieldLabel>
                      <Input
                        id={`organist-credit-${o.key}`}
                        value={o.credit_as}
                        onChange={(e) =>
                          update(o.key, { credit_as: e.target.value })
                        }
                        placeholder="e.g. John Brown, St Mary's"
                      />
                    </Field>
                  </div>
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <Field orientation="horizontal" className="w-auto">
                      <Switch
                        id={`organist-public-${o.key}`}
                        checked={o.credit_publicly}
                        onCheckedChange={(v) =>
                          update(o.key, { credit_publicly: v })
                        }
                      />
                      <FieldLabel
                        htmlFor={`organist-public-${o.key}`}
                        className="font-normal"
                      >
                        OK to name them when a recording is shared
                      </FieldLabel>
                    </Field>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onClick={() => remove(o.key)}
                    >
                      <Trash2Icon data-icon="inline-start" />
                      Remove
                    </Button>
                  </div>
                </FieldGroup>
              </li>
            ))}
          </ul>
        )}
        <p className="text-xs text-muted-foreground">
          Without permission, a shared recording says &ldquo;A Cecilia
          organist&rdquo;. Ask each organist before turning it on.
        </p>
        {choices.length > 1 && (
          <Field className="max-w-sm">
            <FieldLabel htmlFor="organist-current">
              Who&apos;s playing now
            </FieldLabel>
            <Select
              value={current ?? NOBODY}
              onValueChange={(v) =>
                setCurrent(!v || v === NOBODY ? null : String(v))
              }
              items={choices}
            >
              <SelectTrigger id="organist-current" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {choices.map((c) => (
                    <SelectItem key={c.value} value={c.value}>
                      {c.label}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
            <FieldDescription>
              New recordings are credited to them. You can also change this next
              to Record.
            </FieldDescription>
          </Field>
        )}
      </CardContent>
      <CardFooter className="mt-6 flex-wrap gap-2">
        <Button type="button" variant="outline" onClick={add}>
          <PlusIcon data-icon="inline-start" />
          Add an organist
        </Button>
        <Button type="submit" disabled={!changed || missingName || busy}>
          Save organists
        </Button>
      </CardFooter>
    </form>
  )
}
