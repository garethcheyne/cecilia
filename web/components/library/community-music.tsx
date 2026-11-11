"use client"

import * as React from "react"
import {
  CheckIcon,
  PlusIcon,
  RefreshCwIcon,
  SearchIcon,
  SlidersHorizontalIcon,
  WifiOffIcon,
  XIcon,
} from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Drawer,
  DrawerClose,
  DrawerContent,
  DrawerDescription,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from "@/components/ui/drawer"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { Field, FieldLabel } from "@/components/ui/field"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import {
  api,
  call,
  type Catalogue,
  type MusicEntry,
  type TrackSummary,
} from "@/lib/api/client"
import {
  filterMusic,
  isFiltered,
  NO_FILTERS,
  occasions,
  sourceOf,
  type Filters,
  type Kind,
  type Sort,
  type SourceFilter,
} from "@/lib/community-filter"
import { formatClock } from "@/lib/time"

const LICENCE: Record<string, string> = {
  "public-domain": "Public domain",
  "CC0-1.0": "Free to use",
  "CC-BY-4.0": "Credit the arranger",
  "CC-BY-SA-4.0": "Credit and share alike",
}

const SORTS: { value: Sort; label: string }[] = [
  { value: "title", label: "Title, A to Z" },
  { value: "tune", label: "Tune name, A to Z" },
  { value: "shortest", label: "Shortest first" },
  { value: "longest", label: "Longest first" },
]

const SOURCES: { value: SourceFilter; label: string }[] = [
  { value: "all", label: "All sources" },
  { value: "openhymnal", label: "Open Hymnal Project" },
  { value: "cecilia", label: "Cecilia arrangements" },
]

const ANY = "any"
/** Pieces shown at a time; long lists grow on request. */
const PAGE = 50
const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1)

/** Music shared by other churches, ready to add to this library. */
export function CommunityMusic() {
  const { libraryVersion } = useEngine()
  const [cat, setCat] = React.useState<Catalogue | null>(null)
  const [problem, setProblem] = React.useState<string | null>(null)
  const [mine, setMine] = React.useState<Set<string>>(new Set())
  const [filters, setFilters] = React.useState<Filters>(NO_FILTERS)
  const [busy, setBusy] = React.useState<string | null>(null)
  const [limit, setLimit] = React.useState(PAGE)
  const set = (patch: Partial<Filters>) => {
    setFilters((f) => ({ ...f, ...patch }))
    setLimit(PAGE)
  }

  const load = React.useCallback(async (refresh = false) => {
    setProblem(null)
    try {
      setCat(
        await call(
          refresh
            ? api.POST("/api/v1/community/refresh")
            : api.GET("/api/v1/community")
        )
      )
    } catch (e) {
      setProblem((e as Error).message)
    }
  }, [])

  React.useEffect(() => {
    call(api.GET("/api/v1/community"))
      .then(setCat)
      .catch((e: Error) => setProblem(e.message))
  }, [])

  React.useEffect(() => {
    call(api.GET("/api/v1/library"))
      .then((l: TrackSummary[]) => setMine(new Set(l.map((t) => t.id))))
      .catch(() => {})
  }, [libraryVersion])

  const have = React.useCallback(
    (m: MusicEntry) => mine.has(`lib-${m.id}`) || mine.has(`sample-${m.id}`),
    [mine]
  )
  const groups = React.useMemo(() => (cat ? occasions(cat.music) : []), [cat])
  const shown = React.useMemo(
    () => (cat ? filterMusic(cat.music, filters, have) : []),
    [cat, filters, have]
  )
  const manySources = React.useMemo(
    () => new Set(cat?.music.map(sourceOf)).size > 1,
    [cat]
  )

  async function add(id: string, title: string) {
    setBusy(id)
    try {
      await call(
        api.POST("/api/v1/community/music/{id}/add", {
          params: { path: { id } },
        })
      )
      setMine((m) => new Set(m).add(`lib-${id}`))
      toast.success(`Added “${title}” to your library`)
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      setBusy(null)
    }
  }

  if (problem) {
    return (
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <WifiOffIcon />
          </EmptyMedia>
          <EmptyTitle>The shared library isn&apos;t available</EmptyTitle>
          <EmptyDescription>{problem}</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button variant="outline" onClick={() => load(true)}>
            <RefreshCwIcon data-icon="inline-start" />
            Try again
          </Button>
        </EmptyContent>
      </Empty>
    )
  }
  if (!cat) {
    return (
      <div className="flex flex-col gap-2">
        {Array.from({ length: 5 }, (_, i) => (
          <Skeleton key={i} className="h-16" />
        ))}
      </div>
    )
  }

  const occasionItems = [
    { value: ANY, label: "Any occasion" },
    ...groups.flatMap((g) =>
      g.tags.map((t) => ({ value: t, label: capital(t) }))
    ),
  ]
  const activeCount =
    (filters.occasion ? 1 : 0) +
    (filters.kind !== "all" ? 1 : 0) +
    (filters.source !== "all" ? 1 : 0) +
    (filters.hideMine ? 1 : 0)
  const controls = {
    filters,
    onChange: set,
    groups,
    occasionItems,
    manySources,
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <InputGroup className="w-full sm:max-w-md">
          <InputGroupAddon>
            <SearchIcon />
          </InputGroupAddon>
          <InputGroupInput
            placeholder="Title, first line, tune or composer"
            value={filters.query}
            onChange={(e) => set({ query: e.target.value })}
            aria-label="Search shared music"
          />
          {filters.query && (
            <InputGroupAddon align="inline-end">
              <InputGroupButton
                size="icon-xs"
                aria-label="Clear search"
                onClick={() => set({ query: "" })}
              >
                <XIcon />
              </InputGroupButton>
            </InputGroupAddon>
          )}
        </InputGroup>
        <Button variant="ghost" size="sm" onClick={() => load(true)}>
          <RefreshCwIcon data-icon="inline-start" />
          Check for new music
        </Button>
        <Drawer>
          <DrawerTrigger
            render={
              <Button variant="outline" size="sm" className="sm:hidden" />
            }
          >
            <SlidersHorizontalIcon data-icon="inline-start" />
            Filters
            {activeCount > 0 && <Badge className="ml-1">{activeCount}</Badge>}
          </DrawerTrigger>
          <DrawerContent>
            <DrawerHeader>
              <DrawerTitle>Filters</DrawerTitle>
              <DrawerDescription>
                Narrow down the shared music.
              </DrawerDescription>
            </DrawerHeader>
            <div className="max-h-[60svh] overflow-y-auto px-4">
              <FilterControls prefix="shared-phone" {...controls} stacked />
            </div>
            <DrawerFooter className="flex-row">
              <Button
                variant="outline"
                className="flex-1"
                onClick={() =>
                  setFilters((f) => ({ ...NO_FILTERS, sort: f.sort }))
                }
              >
                Clear
              </Button>
              <DrawerClose render={<Button className="flex-1" />}>
                Show {shown.length} pieces
              </DrawerClose>
            </DrawerFooter>
          </DrawerContent>
        </Drawer>
      </div>

      <div className="hidden sm:block">
        <FilterControls prefix="shared" {...controls} />
      </div>
      <div className="flex min-h-8 items-center justify-between gap-3 text-sm text-muted-foreground">
        <span aria-live="polite">
          {isFiltered(filters)
            ? `${shown.length} of ${cat.music.length} pieces`
            : `${cat.music.length} pieces`}
        </span>
        {isFiltered(filters) && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setFilters((f) => ({ ...NO_FILTERS, sort: f.sort }))}
          >
            <XIcon data-icon="inline-start" />
            Clear filters
          </Button>
        )}
      </div>

      {shown.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyTitle>Nothing matches</EmptyTitle>
            <EmptyDescription>
              Try fewer words, or another occasion.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button
              variant="outline"
              onClick={() =>
                setFilters((f) => ({ ...NO_FILTERS, sort: f.sort }))
              }
            >
              Clear filters
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <ul className="flex flex-col divide-y overflow-hidden rounded-xl border bg-card">
          {shown.slice(0, limit).map((m) => (
            <li
              key={m.id}
              className="flex flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3"
            >
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="truncate font-display font-semibold">
                  {m.title}
                </span>
                <span className="truncate text-xs text-muted-foreground">
                  {[
                    m.melody,
                    m.composer,
                    m.verses > 1 && `${m.verses} verses`,
                    formatClock(m.duration_ms),
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
                {m.other_titles && m.other_titles.length > 0 && (
                  <span className="truncate text-xs text-muted-foreground">
                    Also sung as {m.other_titles.join("; ")}
                  </span>
                )}
                {m.source && m.source_url && (
                  <span className="truncate text-xs text-muted-foreground">
                    From the{" "}
                    <a
                      href={m.source_url}
                      target="_blank"
                      rel="noreferrer"
                      className="underline underline-offset-2 hover:text-foreground"
                    >
                      {m.source}
                    </a>
                  </span>
                )}
              </div>
              <Badge
                variant="outline"
                className="hidden sm:inline-flex"
                title={m.licence}
              >
                {LICENCE[m.licence] ?? m.licence}
              </Badge>
              {have(m) ? (
                <Badge variant="secondary">
                  <CheckIcon data-icon="inline-start" />
                  In your library
                </Badge>
              ) : (
                <Button
                  size="sm"
                  variant="outline"
                  disabled={busy === m.id}
                  onClick={() => add(m.id, m.title)}
                >
                  <PlusIcon data-icon="inline-start" />
                  Add
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
      {shown.length > limit && (
        <Button
          variant="outline"
          className="self-center"
          onClick={() => setLimit((l) => l + PAGE)}
        >
          Show {Math.min(PAGE, shown.length - limit)} more (
          {shown.length - limit} left)
        </Button>
      )}
      <p className="text-xs text-muted-foreground">
        Shared by Cecilia users and checked before it&apos;s added. Everything
        here is public domain or freely licensed. Many hymns come from the{" "}
        <a
          href="https://openhymnal.org"
          target="_blank"
          rel="noreferrer"
          className="underline underline-offset-2 hover:text-foreground"
        >
          Open Hymnal Project
        </a>
        , with thanks to Brian J. Dumont and its contributors.
      </p>
    </div>
  )
}

function FilterControls({
  prefix,
  filters,
  onChange,
  groups,
  occasionItems,
  manySources,
  stacked = false,
}: {
  prefix: string
  filters: Filters
  onChange: (patch: Partial<Filters>) => void
  groups: { label: string; tags: string[] }[]
  occasionItems: { value: string; label: string }[]
  manySources: boolean
  stacked?: boolean
}) {
  return (
    <div
      className={
        stacked ? "flex flex-col gap-5" : "flex flex-wrap items-end gap-3"
      }
    >
      <Field className="w-auto">
        <FieldLabel>Type</FieldLabel>
        <ToggleGroup
          value={[filters.kind]}
          onValueChange={(v) => v[0] && onChange({ kind: v[0] as Kind })}
          variant="outline"
          size={stacked ? "default" : "sm"}
          aria-label="Type of music"
        >
          <ToggleGroupItem value="all">Everything</ToggleGroupItem>
          <ToggleGroupItem value="hymns">Hymns</ToggleGroupItem>
          <ToggleGroupItem value="organ">Organ music</ToggleGroupItem>
        </ToggleGroup>
      </Field>
      <Field className={stacked ? "" : "w-48"}>
        <FieldLabel htmlFor={`${prefix}-occasion`}>Occasion</FieldLabel>
        <Select
          value={filters.occasion ?? ANY}
          onValueChange={(v) =>
            onChange({ occasion: !v || v === ANY ? null : String(v) })
          }
          items={occasionItems}
        >
          <SelectTrigger
            id={`${prefix}-occasion`}
            size={stacked ? "default" : "sm"}
            className="w-full"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectItem value={ANY}>Any occasion</SelectItem>
            </SelectGroup>
            {groups.map((g) => (
              <React.Fragment key={g.label}>
                <SelectSeparator />
                <SelectGroup>
                  <SelectLabel>{g.label}</SelectLabel>
                  {g.tags.map((t) => (
                    <SelectItem key={t} value={t}>
                      {capital(t)}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </React.Fragment>
            ))}
          </SelectContent>
        </Select>
      </Field>
      <Field className={stacked ? "" : "w-44"}>
        <FieldLabel htmlFor={`${prefix}-sort`}>Sort</FieldLabel>
        <Select
          value={filters.sort}
          onValueChange={(v) => v && onChange({ sort: v as Sort })}
          items={SORTS}
        >
          <SelectTrigger
            id={`${prefix}-sort`}
            size={stacked ? "default" : "sm"}
            className="w-full"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {SORTS.map((s) => (
                <SelectItem key={s.value} value={s.value}>
                  {s.label}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
      </Field>
      {manySources && (
        <Field className={stacked ? "" : "w-52"}>
          <FieldLabel htmlFor={`${prefix}-source`}>Source</FieldLabel>
          <Select
            value={filters.source}
            onValueChange={(v) => v && onChange({ source: v as SourceFilter })}
            items={SOURCES}
          >
            <SelectTrigger
              id={`${prefix}-source`}
              size={stacked ? "default" : "sm"}
              className="w-full"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {SOURCES.map((s) => (
                  <SelectItem key={s.value} value={s.value}>
                    {s.label}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>
      )}
      <Field orientation="horizontal" className="h-8 w-auto">
        <Switch
          id={`${prefix}-hide-mine`}
          checked={filters.hideMine}
          onCheckedChange={(v) => onChange({ hideMine: v })}
        />
        <FieldLabel htmlFor={`${prefix}-hide-mine`} className="font-normal">
          Hide ones I have
        </FieldLabel>
      </Field>
    </div>
  )
}
