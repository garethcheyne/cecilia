"use client"

import * as React from "react"
import { ExternalLinkIcon, MonitorIcon, MonitorSmartphoneIcon } from "lucide-react"
import { toast } from "sonner"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Field, FieldDescription, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Separator } from "@/components/ui/separator"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { openDisplay, type ScreenCount } from "@/hooks/use-screens"
import { type Displays, type Layout, PANEL_INFO, PANELS, type PanelId, shown } from "@/lib/displays"

const PANEL_ITEMS = PANELS.map((p) => ({ value: p, label: PANEL_INFO[p].label }))

function PanelSelect({ id, label, value, onChange }: { id: string; label: string; value: PanelId; onChange: (p: PanelId) => void }) {
  return (
    <Field>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select value={value} onValueChange={(v) => v && onChange(v as PanelId)} items={PANEL_ITEMS}>
        <SelectTrigger id={id} className="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            {PANELS.map((p) => (
              <SelectItem key={p} value={p}>
                {PANEL_INFO[p].label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
      <FieldDescription>{PANEL_INFO[value].hint}</FieldDescription>
    </Field>
  )
}

function ScreenSettings({
  name,
  legend,
  layout,
  panels,
  onLayout,
  onPanels,
}: {
  name: string
  legend: string
  layout: Layout
  panels: [PanelId, PanelId]
  onLayout: (l: Layout) => void
  onPanels: (p: [PanelId, PanelId]) => void
}) {
  return (
    <FieldSet>
      <FieldLegend>{legend}</FieldLegend>
      <FieldGroup>
        <Field>
          <FieldLabel>Show</FieldLabel>
          <ToggleGroup
            value={[layout]}
            onValueChange={(v) => v[0] && onLayout(v[0] as Layout)}
            variant="outline"
            aria-label={`${legend}: how many panels`}
          >
            <ToggleGroupItem value="single">One panel</ToggleGroupItem>
            <ToggleGroupItem value="split">Two side by side</ToggleGroupItem>
          </ToggleGroup>
        </Field>
        <div className="grid gap-4 sm:grid-cols-2">
          <PanelSelect
            id={`${name}-first`}
            label={layout === "split" ? "Left" : "Panel"}
            value={panels[0]}
            onChange={(p) => onPanels([p, panels[1]])}
          />
          {layout === "split" && (
            <PanelSelect id={`${name}-second`} label="Right" value={panels[1]} onChange={(p) => onPanels([panels[0], p])} />
          )}
        </div>
      </FieldGroup>
    </FieldSet>
  )
}

/** Choose what this screen shows, and open a second screen with something else. */
export function DisplaysDialog({
  displays,
  onChange,
  screens,
  secondOpen,
}: {
  displays: Displays
  onChange: (d: Displays) => void
  screens: ScreenCount
  secondOpen: boolean
}) {
  async function openSecond() {
    const result = await openDisplay(shown(displays.secondLayout, displays.secondPanels))
    if (result === "blocked") {
      toast.error("The browser blocked the new window. Allow pop-ups for Cecilia, then press Open again.")
    } else if (result === "window") {
      toast.success("Opened in a new window", {
        description: "Drag it onto the other screen, then press Full screen in it.",
      })
    }
  }

  return (
    <Dialog>
      <DialogTrigger render={<Button variant="outline" size="sm" />}>
        <MonitorSmartphoneIcon data-icon="inline-start" />
        <span className="hidden sm:inline">Displays</span>
      </DialogTrigger>
      <DialogContent className="max-h-[90svh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Displays</DialogTitle>
          <DialogDescription>
            Choose what each screen shows during the service. Every screen follows the same service, so pressing Play
            on one is seen on all.
          </DialogDescription>
        </DialogHeader>

        <ScreenSettings
          name="main"
          legend="This screen"
          layout={displays.layout}
          panels={displays.panels}
          onLayout={(layout) => onChange({ ...displays, layout })}
          onPanels={(panels) => onChange({ ...displays, panels })}
        />

        <Separator />

        <ScreenSettings
          name="second"
          legend="Second screen"
          layout={displays.secondLayout}
          panels={displays.secondPanels}
          onLayout={(secondLayout) => onChange({ ...displays, secondLayout })}
          onPanels={(secondPanels) => onChange({ ...displays, secondPanels })}
        />

        {screens === "many" ? (
          <Alert>
            <MonitorIcon />
            <AlertTitle className="flex items-center gap-2">
              More than one screen found <Badge variant="secondary">2+ screens</Badge>
            </AlertTitle>
            <AlertDescription>
              Open sends the second screen to the other monitor. The browser may ask permission to place windows the
              first time.
            </AlertDescription>
          </Alert>
        ) : (
          <Alert>
            <MonitorIcon />
            <AlertTitle>{screens === "one" ? "Only one screen found" : "Can’t tell how many screens there are"}</AlertTitle>
            <AlertDescription>
              {screens === "one"
                ? "Plug in another screen to use one, or open it as a window on this screen or on another device."
                : "This browser doesn’t say. Open it as a window and drag it onto the other screen, or open Cecilia’s address on another device."}
            </AlertDescription>
          </Alert>
        )}

        <DialogFooter>
          {secondOpen && (
            <p className="mr-auto self-center text-sm text-muted-foreground">
              The second screen is open and follows what you choose here.
            </p>
          )}
          <Button onClick={openSecond}>
            <ExternalLinkIcon data-icon="inline-start" />
            {screens === "many" ? "Open on second screen" : "Open in a new window"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
