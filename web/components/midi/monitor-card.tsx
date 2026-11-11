"use client"

import { ActivityIcon, EraserIcon, PauseIcon, PlayIcon } from "lucide-react"

import { useMonitor } from "@/components/engine-provider"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { describeMidi, toHex } from "@/lib/midi"

export function MonitorCard() {
  const { events, skipped, paused, setPaused, clearEvents } = useMonitor()

  return (
    <Card className="flex min-h-0 flex-col">
      <CardHeader>
        <CardTitle>Live monitor</CardTitle>
        <CardDescription>
          MIDI going in and out of the server, newest first.
          {skipped > 0 && ` ${skipped} messages skipped while this page was busy.`}
        </CardDescription>
        <CardAction className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => setPaused(!paused)}>
            {paused ? <PlayIcon data-icon="inline-start" /> : <PauseIcon data-icon="inline-start" />}
            {paused ? "Resume" : "Pause"}
          </Button>
          <Button variant="outline" size="sm" onClick={clearEvents} disabled={events.length === 0}>
            <EraserIcon data-icon="inline-start" />
            Clear
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="min-h-0 flex-1">
        {events.length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <ActivityIcon />
              </EmptyMedia>
              <EmptyTitle>{paused ? "Monitor paused" : "Waiting for MIDI"}</EmptyTitle>
              <EmptyDescription>
                Press a key or draw a stop on the console to see it here.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <ScrollArea className="h-96">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-24">Time</TableHead>
                  <TableHead className="w-16">Dir</TableHead>
                  <TableHead className="w-12">Ch</TableHead>
                  <TableHead>Message</TableHead>
                  <TableHead className="hidden md:table-cell">Port</TableHead>
                  <TableHead className="hidden text-right sm:table-cell">Bytes</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {events.map((e) => {
                  const decoded = describeMidi(e.bytes)
                  return (
                    <TableRow key={e.id}>
                      <TableCell className="font-mono text-xs tabular-nums text-muted-foreground">
                        {(e.t_us / 1_000_000).toFixed(3)}
                      </TableCell>
                      <TableCell>
                        <Badge variant={e.direction === "in" ? "secondary" : "outline"}>
                          {e.direction === "in" ? "In" : "Out"}
                        </Badge>
                      </TableCell>
                      <TableCell className="tabular-nums">{decoded.channel ?? "–"}</TableCell>
                      <TableCell>{decoded.summary}</TableCell>
                      <TableCell className="hidden max-w-48 truncate text-muted-foreground md:table-cell">
                        {e.port}
                      </TableCell>
                      <TableCell className="hidden text-right font-mono text-xs sm:table-cell">
                        {toHex(e.bytes)}
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </ScrollArea>
        )}
      </CardContent>
    </Card>
  )
}
