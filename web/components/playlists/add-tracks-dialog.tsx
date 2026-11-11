"use client"

import * as React from "react"
import { SearchIcon } from "lucide-react"

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
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import type { TrackSummary } from "@/lib/api/client"
import { formatClock } from "@/lib/time"

/** Pick pieces from the library, in the order they're ticked. */
export function AddTracksDialog({
  open,
  onOpenChange,
  library,
  onAdd,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  library: TrackSummary[]
  onAdd: (ids: string[]) => void
}) {
  const [query, setQuery] = React.useState("")
  const [picked, setPicked] = React.useState<string[]>([])
  const [wasOpen, setWasOpen] = React.useState(false)
  if (open && !wasOpen) {
    setWasOpen(true)
    setPicked([])
    setQuery("")
  }
  if (!open && wasOpen) setWasOpen(false)

  const q = query.trim().toLowerCase()
  const shown = library.filter(
    (t) => !q || [t.info.title, t.info.composer, t.info.melody].some((f) => f?.toLowerCase().includes(q))
  )
  const toggle = (id: string, on: boolean) =>
    setPicked((p) => (on ? [...p.filter((x) => x !== id), id] : p.filter((x) => x !== id)))

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Add music</DialogTitle>
          <DialogDescription>Tick pieces in the order you want them.</DialogDescription>
        </DialogHeader>
        <InputGroup>
          <InputGroupAddon>
            <SearchIcon />
          </InputGroupAddon>
          <InputGroupInput
            placeholder="Search the library"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="Search the library"
          />
        </InputGroup>
        <ul className="-mx-1 flex max-h-[50svh] flex-col overflow-y-auto">
          {shown.map((t) => {
            const n = picked.indexOf(t.id)
            return (
              <li key={t.id}>
                <label className="flex cursor-pointer items-center gap-3 rounded-lg px-2 py-2 hover:bg-muted">
                  <Checkbox checked={n >= 0} onCheckedChange={(on) => toggle(t.id, !!on)} />
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate font-medium">{t.info.title}</span>
                    <span className="truncate text-xs text-muted-foreground">
                      {[t.info.melody || t.info.composer, formatClock(t.duration_ms), t.verses > 1 && `${t.verses} verses`]
                        .filter(Boolean)
                        .join(" · ")}
                    </span>
                  </span>
                  {n >= 0 && (
                    <span className="flex size-6 items-center justify-center rounded-full bg-primary text-xs font-semibold text-primary-foreground tabular-nums">
                      {n + 1}
                    </span>
                  )}
                </label>
              </li>
            )
          })}
          {shown.length === 0 && <li className="py-6 text-center text-sm text-muted-foreground">Nothing matches.</li>}
        </ul>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            disabled={picked.length === 0}
            onClick={() => {
              onAdd(picked)
              onOpenChange(false)
            }}
          >
            {picked.length === 1 ? "Add 1 piece" : `Add ${picked.length || ""} pieces`.replace("  ", " ")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
