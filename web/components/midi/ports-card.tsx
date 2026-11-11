"use client"

import * as React from "react"
import { CableIcon, RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { useEngine } from "@/components/engine-provider"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSeparator,
  FieldSet,
} from "@/components/ui/field"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { api, call, type MidiConfig, type PortList } from "@/lib/api/client"

const NO_OUTPUT = "__none__"

/** Available ports plus any saved ones that have gone missing, so they stay visible. */
function withSaved(
  available: string[],
  saved: string[]
): { name: string; present: boolean }[] {
  const all = [...available, ...saved.filter((s) => !available.includes(s))]
  return all.map((name) => ({ name, present: available.includes(name) }))
}

function fetchPortsAndConfig(): Promise<[PortList, MidiConfig]> {
  return Promise.all([
    call(api.GET("/api/v1/midi/ports")),
    call(api.GET("/api/v1/midi/config")),
  ])
}

export function PortsCard() {
  const { connected } = useEngine()
  const [ports, setPorts] = React.useState<PortList | null>(null)
  const [saved, setSaved] = React.useState<MidiConfig | null>(null)
  const [inputs, setInputs] = React.useState<string[]>([])
  const [output, setOutput] = React.useState<string>(NO_OUTPUT)
  const [loading, setLoading] = React.useState(false)
  const [saving, setSaving] = React.useState(false)

  const apply = React.useCallback(([p, c]: [PortList, MidiConfig]) => {
    setPorts(p)
    setSaved(c)
    setInputs(c.inputs ?? [])
    setOutput(c.output ?? NO_OUTPUT)
  }, [])

  // (Re)load whenever the server connection comes up.
  React.useEffect(() => {
    if (!connected) return
    let cancelled = false
    fetchPortsAndConfig()
      .then((result) => !cancelled && apply(result))
      .catch((e: Error) => toast.error(e.message))
    return () => {
      cancelled = true
    }
  }, [connected, apply])

  function refresh() {
    setLoading(true)
    // Only re-list ports; keep any unsaved selection.
    call(api.GET("/api/v1/midi/ports"))
      .then(setPorts)
      .catch((e: Error) => toast.error(e.message))
      .finally(() => setLoading(false))
  }

  const savedOutput = saved?.output ?? NO_OUTPUT
  const dirty =
    saved !== null &&
    (output !== savedOutput ||
      inputs.length !== (saved.inputs ?? []).length ||
      inputs.some((i) => !(saved.inputs ?? []).includes(i)))

  async function save() {
    setSaving(true)
    try {
      const config: MidiConfig = {
        inputs,
        output: output === NO_OUTPUT ? null : output,
      }
      const status = await call(
        api.PUT("/api/v1/midi/config", { body: config })
      )
      setSaved(config)
      if (status.missing.length > 0) {
        toast.warning(`Saved, but could not open: ${status.missing.join(", ")}`)
      } else {
        toast.success("MIDI connections updated")
      }
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      setSaving(false)
    }
  }

  const inputRows = withSaved(ports?.inputs ?? [], saved?.inputs ?? [])
  const outputRows = withSaved(
    ports?.outputs ?? [],
    saved?.output ? [saved.output] : []
  )

  return (
    <Card>
      <CardHeader>
        <CardTitle>Ports</CardTitle>
        <CardDescription className="flex flex-col gap-1.5">
          <span>
            Choose the connection Cecilia hears the organ on, and the one it
            plays the organ through.
          </span>
          <details className="text-xs">
            <summary className="cursor-pointer underline-offset-2 select-none hover:underline">
              Using a virtual organ on a computer?
            </summary>
            <p className="pt-1">
              With Hauptwerk or GrandOrgue, connect Cecilia using virtual MIDI
              cables (for example loopMIDI on Windows), then choose those cables
              here.
            </p>
          </details>
        </CardDescription>
        <CardAction>
          <Button
            variant="outline"
            size="sm"
            onClick={refresh}
            disabled={!connected || loading}
          >
            {loading ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <RefreshCwIcon data-icon="inline-start" />
            )}
            Refresh
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        {ports === null ? (
          <div className="flex flex-col gap-3">
            <Skeleton className="h-5 w-1/3" />
            <Skeleton className="h-5 w-2/3" />
            <Skeleton className="h-5 w-1/2" />
          </div>
        ) : inputRows.length === 0 && outputRows.length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <CableIcon />
              </EmptyMedia>
              <EmptyTitle>No MIDI ports found</EmptyTitle>
              <EmptyDescription>
                Plug in the organ&apos;s MIDI cable or USB lead, then press
                Refresh.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <FieldGroup>
            <FieldSet>
              <FieldLegend variant="label">Organ in</FieldLegend>
              <FieldDescription>
                Where Cecilia listens to the organ. You can choose more than
                one.
              </FieldDescription>
              <FieldGroup data-slot="checkbox-group">
                {inputRows.length === 0 && (
                  <FieldDescription>Nothing to listen to yet.</FieldDescription>
                )}
                {inputRows.map(({ name, present }) => (
                  <Field
                    key={name}
                    orientation="horizontal"
                    data-disabled={!connected || undefined}
                  >
                    <Checkbox
                      id={`in-${name}`}
                      checked={inputs.includes(name)}
                      disabled={!connected}
                      onCheckedChange={(checked) =>
                        setInputs((prev) =>
                          checked
                            ? [...prev, name]
                            : prev.filter((p) => p !== name)
                        )
                      }
                    />
                    <FieldLabel htmlFor={`in-${name}`} className="font-normal">
                      {name}
                    </FieldLabel>
                    {!present && <Badge variant="destructive">Not found</Badge>}
                  </Field>
                ))}
              </FieldGroup>
            </FieldSet>
            <FieldSeparator />
            <FieldSet>
              <FieldLegend variant="label">Organ out</FieldLegend>
              <FieldDescription>
                Where Cecilia plays the organ. Choose one.
              </FieldDescription>
              <RadioGroup
                value={output}
                onValueChange={(v) => setOutput(String(v))}
                disabled={!connected}
              >
                <Field orientation="horizontal">
                  <RadioGroupItem value={NO_OUTPUT} id="out-none" />
                  <FieldLabel htmlFor="out-none" className="font-normal">
                    None
                  </FieldLabel>
                </Field>
                {outputRows.map(({ name, present }) => (
                  <Field key={name} orientation="horizontal">
                    <RadioGroupItem value={name} id={`out-${name}`} />
                    <FieldLabel htmlFor={`out-${name}`} className="font-normal">
                      {name}
                    </FieldLabel>
                    {!present && <Badge variant="destructive">Not found</Badge>}
                  </Field>
                ))}
              </RadioGroup>
            </FieldSet>
          </FieldGroup>
        )}
      </CardContent>
      <CardFooter className="justify-end gap-2">
        {dirty && (
          <Button
            variant="ghost"
            onClick={() => {
              setInputs(saved?.inputs ?? [])
              setOutput(savedOutput)
            }}
          >
            Discard
          </Button>
        )}
        <Button onClick={save} disabled={!connected || !dirty || saving}>
          {saving && <Spinner data-icon="inline-start" />}
          Save &amp; connect
        </Button>
      </CardFooter>
    </Card>
  )
}
