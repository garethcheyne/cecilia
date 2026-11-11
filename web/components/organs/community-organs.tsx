"use client"

import * as React from "react"
import { useRouter } from "next/navigation"
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  SearchIcon,
  SparklesIcon,
} from "lucide-react"
import { toast } from "sonner"

import { CONSOLE_ORDER } from "@/components/organ/divisions"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
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
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import {
  api,
  call,
  type OrganDefinition,
  type OrganEntry,
  type Stop,
} from "@/lib/api/client"
import { formatFootage } from "@/lib/organ"
import {
  makers,
  models,
  organTitle,
  searchOrgans,
  type Model,
} from "@/lib/organ-catalogue"

const CONFIDENCE: Record<string, { label: string; hint: string }> = {
  template: {
    label: "Template",
    hint: "Worked out from the maker's published stop list. Check each stop on your organ.",
  },
  learned: {
    label: "Learned",
    hint: "Every stop was captured on a real organ of this model.",
  },
  verified: { label: "Verified", hint: "Learned, then tested with playback." },
}

type Step =
  | { kind: "makers" }
  | { kind: "models"; maker: string }
  | { kind: "variants"; model: Model }
  | { kind: "preview"; setup: OrganEntry }

/**
 * Set up the church's organ from the shared library: choose the maker, then
 * the model, then (if there's more than one) the variant, check the stops,
 * and add it. Setups that are already built in are left out.
 */
export function CommunityOrgans({
  builtinIds,
  onAdded,
}: {
  builtinIds: string[] | null
  onAdded?: () => void
}) {
  const [organs, setOrgans] = React.useState<OrganEntry[] | null>(null)
  const [problem, setProblem] = React.useState<string | null>(null)
  const [open, setOpen] = React.useState(false)

  React.useEffect(() => {
    call(api.GET("/api/v1/community"))
      .then((c) => setOrgans(c.organs))
      .catch((e: Error) => setProblem(e.message))
  }, [])

  const shown = React.useMemo(
    () =>
      organs && builtinIds
        ? organs.filter((o) => !builtinIds.includes(o.id))
        : null,
    [organs, builtinIds]
  )
  const makerCount = shown ? makers(shown).length : 0

  return (
    <Card>
      <CardHeader>
        <CardTitle className="font-display text-lg">
          Set up your church&apos;s organ
        </CardTitle>
        <CardDescription>
          {problem
            ? problem
            : shown === null
              ? "Looking in the shared library…"
              : shown.length === 0
                ? "No organ setups have been shared yet. Make a copy of the closest organ above and teach Cecilia its stops."
                : `Choose the maker and model, and Cecilia loads its stops. ${shown.length} setups from ${makerCount} ${makerCount === 1 ? "maker" : "makers"} so far.`}
        </CardDescription>
        <CardAction>
          <Button
            onClick={() => setOpen(true)}
            disabled={!shown || shown.length === 0}
          >
            <SparklesIcon data-icon="inline-start" />
            Find my organ
          </Button>
        </CardAction>
      </CardHeader>
      {shown && shown.length > 0 && (
        <SetupDialog
          open={open}
          onOpenChange={setOpen}
          organs={shown}
          onAdded={onAdded}
        />
      )}
    </Card>
  )
}

function SetupDialog({
  open,
  onOpenChange,
  organs,
  onAdded,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  organs: OrganEntry[]
  onAdded?: () => void
}) {
  const router = useRouter()
  const [step, setStep] = React.useState<Step>({ kind: "makers" })
  const [query, setQuery] = React.useState("")
  const [busy, setBusy] = React.useState(false)

  // Start again each time it opens.
  const [wasOpen, setWasOpen] = React.useState(open)
  if (open !== wasOpen) {
    setWasOpen(open)
    if (open) {
      setStep({ kind: "makers" })
      setQuery("")
    }
  }

  function chooseModel(m: Model) {
    setStep(
      m.setups.length === 1
        ? { kind: "preview", setup: m.setups[0] }
        : { kind: "variants", model: m }
    )
  }

  function back() {
    if (step.kind === "models") setStep({ kind: "makers" })
    else if (step.kind === "variants")
      setStep({ kind: "models", maker: step.model.setups[0].meta.maker })
    else if (step.kind === "preview") {
      const m = models(organs, step.setup.meta.maker)
        .flatMap((g) => g.models)
        .find((x) => x.setups.some((s) => s.id === step.setup.id))
      setStep(
        m && m.setups.length > 1
          ? { kind: "variants", model: m }
          : { kind: "models", maker: step.setup.meta.maker }
      )
    }
  }

  async function add(setup: OrganEntry, use: boolean) {
    setBusy(true)
    try {
      const added = await call(
        api.POST("/api/v1/community/organs/{id}/add", {
          params: { path: { id: setup.id } },
        })
      )
      if (use)
        await call(
          api.PUT("/api/v1/simulator/organ", { body: { organ_id: added.id } })
        )
      onAdded?.()
      onOpenChange(false)
      toast.success(use ? `Now using ${added.name}` : `Added ${added.name}`, {
        description:
          setup.meta.confidence === "template"
            ? "Check each stop on your organ with Learn."
            : undefined,
        action: {
          label: "Open",
          onClick: () =>
            router.push(`/organs/edit/?id=${encodeURIComponent(added.id)}`),
        },
      })
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const found = searchOrgans(organs, query)
  const title =
    step.kind === "makers"
      ? "Who made your organ?"
      : step.kind === "models"
        ? `Which ${step.maker}?`
        : step.kind === "variants"
          ? `Which ${step.model.setups[0].meta.maker} ${step.model.model}?`
          : organTitle(step.setup)
  const description =
    step.kind === "makers"
      ? "The maker's name is usually on the console, above the top keyboard."
      : step.kind === "models"
        ? "The model is often on a plate by the keys, or in the owner's manual."
        : step.kind === "variants"
          ? "This model was made in more than one form. Choose the one that matches your console."
          : step.setup.meta.variant ||
            "Check these stops against your console before you add it."

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="gap-4 sm:max-w-xl">
        <DialogHeader>
          <DialogTitle className="font-display">{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>

        <div className="-mx-1 max-h-[60svh] min-h-48 overflow-y-auto px-1">
          {step.kind === "makers" && (
            <div className="flex flex-col gap-3">
              <InputGroup>
                <InputGroupAddon>
                  <SearchIcon />
                </InputGroupAddon>
                <InputGroupInput
                  placeholder="Or type the model, e.g. Studio 170"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  aria-label="Search organ models"
                />
              </InputGroup>
              {query.trim() ? (
                found.length ? (
                  <ChoiceList>
                    {found.map((o) => (
                      <Choice
                        key={o.id}
                        onClick={() => setStep({ kind: "preview", setup: o })}
                        title={organTitle(o)}
                        detail={setupDetail(o)}
                      />
                    ))}
                  </ChoiceList>
                ) : (
                  <NotListed />
                )
              ) : (
                <>
                  <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                    {makers(organs).map((m) => (
                      <Button
                        key={m.name}
                        variant="outline"
                        className="h-auto flex-col items-start gap-0.5 px-3 py-2.5 text-left"
                        onClick={() =>
                          setStep({ kind: "models", maker: m.name })
                        }
                      >
                        <span className="font-display text-base">{m.name}</span>
                        <span className="text-xs font-normal text-muted-foreground">
                          {m.models} {m.models === 1 ? "model" : "models"}
                        </span>
                      </Button>
                    ))}
                  </div>
                  <NotListed />
                </>
              )}
            </div>
          )}

          {step.kind === "models" && (
            <div className="flex flex-col gap-4">
              {models(organs, step.maker).map((g) => (
                <section
                  key={g.series || "other"}
                  className="flex flex-col gap-1.5"
                >
                  {g.series && (
                    <h3 className="text-xs font-medium text-muted-foreground">
                      {g.series} series
                    </h3>
                  )}
                  <ChoiceList>
                    {g.models.map((m) => (
                      <Choice
                        key={m.model}
                        onClick={() => chooseModel(m)}
                        title={m.model}
                        detail={
                          m.setups.length > 1
                            ? `${m.setups.length} versions`
                            : setupDetail(m.setups[0])
                        }
                      />
                    ))}
                  </ChoiceList>
                </section>
              ))}
            </div>
          )}

          {step.kind === "variants" && (
            <ChoiceList>
              {step.model.setups.map((s) => (
                <Choice
                  key={s.id}
                  onClick={() => setStep({ kind: "preview", setup: s })}
                  title={s.meta.variant || "Standard"}
                  detail={setupDetail(s)}
                />
              ))}
            </ChoiceList>
          )}

          {step.kind === "preview" && <Preview setup={step.setup} />}
        </div>

        {step.kind !== "makers" && (
          <DialogFooter className="gap-2 sm:justify-between">
            <Button variant="ghost" onClick={back} disabled={busy}>
              <ChevronLeftIcon data-icon="inline-start" />
              Back
            </Button>
            {step.kind === "preview" && (
              <div className="flex flex-col-reverse gap-2 sm:flex-row">
                <Button
                  variant="outline"
                  onClick={() => add(step.setup, false)}
                  disabled={busy}
                >
                  Add only
                </Button>
                <Button onClick={() => add(step.setup, true)} disabled={busy}>
                  {busy && <Spinner data-icon="inline-start" />}
                  Add and use this organ
                </Button>
              </div>
            )}
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  )
}

/** "Gedackt 8" → "Gedackt": the pitch is shown beside it. */
function stopName(s: Stop): string {
  const name = s.full_name ?? s.names.join(" ")
  return s.equivalent.type === "voice"
    ? name.replace(/\s+\d+(\s+\d+\/\d+)?['′]?$/, "")
    : name
}

function setupDetail(o: OrganEntry): string {
  const keyboards = Math.max(o.divisions - 1, 1)
  return [
    o.meta.variant,
    `${keyboards} ${keyboards === 1 ? "keyboard" : "keyboards"} and pedals`,
    `${o.stops} stops`,
    o.meta.years,
  ]
    .filter(Boolean)
    .join(" · ")
}

function ChoiceList({ children }: { children: React.ReactNode }) {
  return (
    <ul className="flex flex-col divide-y overflow-hidden rounded-lg border">
      {children}
    </ul>
  )
}

function Choice({
  title,
  detail,
  onClick,
}: {
  title: string
  detail: string
  onClick: () => void
}) {
  return (
    <li>
      <button
        type="button"
        onClick={onClick}
        className="flex w-full items-center gap-3 px-3 py-2.5 text-left outline-none hover:bg-muted focus-visible:bg-muted"
      >
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">{title}</span>
          <span className="truncate text-xs text-muted-foreground">
            {detail}
          </span>
        </span>
        <ChevronRightIcon
          className="size-4 shrink-0 text-muted-foreground"
          aria-hidden
        />
      </button>
    </li>
  )
}

function NotListed() {
  return (
    <p className="text-sm text-muted-foreground">
      Not listed? Close this, make a copy of the organ closest to yours, and
      teach Cecilia its stops with Learn every stop. Once it works, it can be
      shared so the next church with that model doesn&apos;t have to.
    </p>
  )
}

function Preview({ setup }: { setup: OrganEntry }) {
  const [organ, setOrgan] = React.useState<OrganDefinition | null>(null)
  const [problem, setProblem] = React.useState<string | null>(null)

  React.useEffect(() => {
    call(
      api.GET("/api/v1/community/organs/{id}", {
        params: { path: { id: setup.id } },
      })
    )
      .then(setOrgan)
      .catch((e: Error) => setProblem(e.message))
  }, [setup.id])

  const c = CONFIDENCE[setup.meta.confidence]
  const divisions = organ
    ? [...organ.divisions].sort(
        (a, b) => CONSOLE_ORDER.indexOf(a.role) - CONSOLE_ORDER.indexOf(b.role)
      )
    : []

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
        <Badge
          variant={
            setup.meta.confidence === "template" ? "outline" : "secondary"
          }
        >
          {c?.label ?? setup.meta.confidence}
        </Badge>
        <span>{c?.hint}</span>
      </div>
      {(setup.meta.notes ?? []).length > 0 && (
        <Alert>
          <AlertTitle>Before you start</AlertTitle>
          <AlertDescription>
            <ul className="list-disc pl-4">
              {(setup.meta.notes ?? []).map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      )}
      {problem ? (
        <p className="text-sm text-muted-foreground">{problem}</p>
      ) : !organ ? (
        <Skeleton className="h-40" />
      ) : (
        <div className="grid gap-4 sm:grid-cols-2">
          {divisions.map((d) => {
            const stops = organ.stops.filter((s) => s.division === d.role)
            return (
              <section key={d.role} className="flex flex-col gap-1">
                <h3 className="font-display font-semibold">{d.name}</h3>
                <ul className="text-sm">
                  {stops.map((s) => (
                    <li
                      key={s.id}
                      className="flex justify-between gap-3 py-0.5"
                    >
                      <span className="truncate">{stopName(s)}</span>
                      {s.equivalent.type === "voice" && (
                        <span className="shrink-0 text-muted-foreground tabular-nums">
                          {formatFootage(s.equivalent.footage)}
                        </span>
                      )}
                    </li>
                  ))}
                </ul>
              </section>
            )
          })}
        </div>
      )}
    </div>
  )
}
