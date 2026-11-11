"use client"

import * as React from "react"
import { CheckIcon, PlusIcon, RefreshCwIcon, SearchIcon, WifiOffIcon } from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Skeleton } from "@/components/ui/skeleton"
import { api, call, type Catalogue, type TrackSummary } from "@/lib/api/client"
import { formatClock } from "@/lib/time"

const LICENCE: Record<string, string> = {
  "public-domain": "Public domain",
  "CC0-1.0": "Free to use",
  "CC-BY-4.0": "Credit the arranger",
  "CC-BY-SA-4.0": "Credit and share alike",
}

/** Music shared by other churches, ready to add to this library. */
export function CommunityMusic() {
  const { libraryVersion } = useEngine()
  const [cat, setCat] = React.useState<Catalogue | null>(null)
  const [problem, setProblem] = React.useState<string | null>(null)
  const [mine, setMine] = React.useState<Set<string>>(new Set())
  const [query, setQuery] = React.useState("")
  const [tag, setTag] = React.useState<string | null>(null)
  const [busy, setBusy] = React.useState<string | null>(null)

  const load = React.useCallback(async (refresh = false) => {
    setProblem(null)
    try {
      setCat(await call(refresh ? api.POST("/api/v1/community/refresh") : api.GET("/api/v1/community")))
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

  async function add(id: string, title: string) {
    setBusy(id)
    try {
      await call(api.POST("/api/v1/community/music/{id}/add", { params: { path: { id } } }))
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

  const tags = [...new Set(cat.music.flatMap((m) => m.tags))].sort()
  const q = query.trim().toLowerCase()
  const shown = cat.music.filter(
    (m) =>
      (!tag || m.tags.includes(tag)) &&
      (!q || [m.title, m.composer, m.melody, ...(m.other_titles ?? [])].some((f) => f.toLowerCase().includes(q)))
  )

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <InputGroup className="w-full sm:max-w-sm">
          <InputGroupAddon>
            <SearchIcon />
          </InputGroupAddon>
          <InputGroupInput
            placeholder="Search shared music"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="Search shared music"
          />
        </InputGroup>
        <Button variant="ghost" size="sm" onClick={() => load(true)}>
          <RefreshCwIcon data-icon="inline-start" />
          Check for new music
        </Button>
      </div>
      <div className="flex flex-wrap gap-1.5" role="group" aria-label="Filter by occasion">
        <Button size="xs" variant={tag === null ? "default" : "outline"} onClick={() => setTag(null)}>
          Everything
        </Button>
        {tags.map((t) => (
          <Button key={t} size="xs" variant={tag === t ? "default" : "outline"} onClick={() => setTag(tag === t ? null : t)}>
            {t.replace(/-/g, " ")}
          </Button>
        ))}
      </div>
      <ul className="flex flex-col divide-y overflow-hidden rounded-xl border bg-card">
        {shown.map((m) => {
          const have = mine.has(`lib-${m.id}`) || mine.has(`sample-${m.id}`)
          return (
            <li key={m.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3">
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="truncate font-display font-semibold">{m.title}</span>
                <span className="truncate text-xs text-muted-foreground">
                  {[m.melody, m.composer, m.verses > 1 && `${m.verses} verses`, formatClock(m.duration_ms)]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
                {m.other_titles && m.other_titles.length > 0 && (
                  <span className="truncate text-xs text-muted-foreground">Also sung as {m.other_titles.join("; ")}</span>
                )}
                {m.source && m.source_url && (
                  <span className="truncate text-xs text-muted-foreground">
                    From the{" "}
                    {m.source_url ? (
                      <a href={m.source_url} target="_blank" rel="noreferrer" className="underline underline-offset-2 hover:text-foreground">
                        {m.source}
                      </a>
                    ) : (
                      m.source
                    )}
                  </span>
                )}
              </div>
              <Badge variant="outline" className="hidden sm:inline-flex" title={m.licence}>
                {LICENCE[m.licence] ?? m.licence}
              </Badge>
              {have ? (
                <Badge variant="secondary">
                  <CheckIcon data-icon="inline-start" />
                  In your library
                </Badge>
              ) : (
                <Button size="sm" variant="outline" disabled={busy === m.id} onClick={() => add(m.id, m.title)}>
                  <PlusIcon data-icon="inline-start" />
                  Add
                </Button>
              )}
            </li>
          )
        })}
        {shown.length === 0 && <li className="px-4 py-8 text-center text-sm text-muted-foreground">Nothing matches.</li>}
      </ul>
      <p className="text-xs text-muted-foreground">
        Shared by Cecilia users and checked before it&apos;s added. Everything here is public domain or freely licensed.
        Many hymns come from the{" "}
        <a href="https://openhymnal.org" target="_blank" rel="noreferrer" className="underline underline-offset-2 hover:text-foreground">
          Open Hymnal Project
        </a>
        , with thanks to Brian J. Dumont and its contributors.
      </p>
    </div>
  )
}
