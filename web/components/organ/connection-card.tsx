import { CableIcon } from "lucide-react"

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import type { OrganDefinition } from "@/lib/api/client"

/** How to cable this organ, plus anything to check before trusting its definition. */
export function ConnectionCard({ organ }: { organ: OrganDefinition }) {
  const c = organ.connection
  if (!c) return null
  const sockets = [
    c.record_from && { label: "Record from", socket: c.record_from, hint: "organ → your MIDI interface's input" },
    c.play_to && { label: "Play back into", socket: c.play_to, hint: "your MIDI interface's output → organ" },
  ].filter((s): s is { label: string; socket: string; hint: string } => !!s)

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <CableIcon className="text-muted-foreground" aria-hidden />
          Connecting this organ
        </CardTitle>
        <CardDescription>Use the sockets as labelled on the organ.</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {sockets.length > 0 && (
          <dl className="grid gap-3 sm:grid-cols-2">
            {sockets.map((s) => (
              <div key={s.label} className="flex flex-col gap-1 rounded-lg border bg-muted/40 p-3">
                <dt className="text-sm text-muted-foreground">{s.label}</dt>
                <dd className="font-display text-lg font-semibold">{s.socket}</dd>
                <dd className="text-xs text-muted-foreground">{s.hint}</dd>
              </div>
            ))}
          </dl>
        )}
        {c.notes && c.notes.length > 0 && (
          <ul className="flex list-disc flex-col gap-1.5 pl-5 text-sm text-muted-foreground marker:text-primary">
            {c.notes.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  )
}
