"use client"

import * as React from "react"
import { CheckIcon, CopyIcon, HourglassIcon, MicIcon } from "lucide-react"
import { toast } from "sonner"

import { PageBody } from "@/components/page-body"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
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
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group"
import { Skeleton } from "@/components/ui/skeleton"
import { API_URL, api, call } from "@/lib/api/client"

/** Things the assistant understands today, phrased as an organist would say them. */
const EXAMPLES = [
  "Draw the Open Diapason and the Principal 4",
  "Add Swell to Great",
  "Swell pedal half open",
  "General cancel",
  "Record this",
  "Stop",
  "Play it back",
  "Silence",
]

function useMcpUrl() {
  return React.useSyncExternalStore(
    () => () => {},
    () => `${API_URL || window.location.origin}/mcp`,
    () => ""
  )
}

function CopyField({
  id,
  value,
  label,
}: {
  id: string
  value: string
  label: string
}) {
  const [copied, setCopied] = React.useState(false)
  const input = React.useRef<HTMLInputElement>(null)

  async function copy() {
    try {
      await navigator.clipboard.writeText(value)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch {
      // The clipboard API needs https or localhost; fall back to selecting it.
      input.current?.select()
      toast("Selected. Copy it with your device's copy command.")
    }
  }

  return (
    <InputGroup>
      <InputGroupInput
        id={id}
        ref={input}
        readOnly
        value={value}
        className="font-mono text-xs"
      />
      <InputGroupAddon align="inline-end">
        <InputGroupButton
          size="icon-xs"
          onClick={copy}
          aria-label={`Copy ${label}`}
        >
          {copied ? <CheckIcon /> : <CopyIcon />}
        </InputGroupButton>
      </InputGroupAddon>
    </InputGroup>
  )
}

/** A long command that wraps instead of being cut off, with a copy button. */
function CopyBlock({ value, label }: { value: string; label: string }) {
  const [copied, setCopied] = React.useState(false)

  async function copy() {
    try {
      await navigator.clipboard.writeText(value)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch {
      toast(
        "Copying needs a secure connection. Select the text and copy it instead."
      )
    }
  }

  return (
    <div className="flex items-start gap-2 rounded-lg border bg-muted/40 py-2 pr-1.5 pl-3">
      <code className="min-w-0 flex-1 font-mono text-xs leading-relaxed break-all select-all">
        {value}
      </code>
      <Button
        variant="ghost"
        size="icon-xs"
        onClick={copy}
        aria-label={`Copy ${label}`}
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </Button>
    </div>
  )
}

export function AssistantSetup() {
  const [saved, setSaved] = React.useState<string | null>(null)
  const [draft, setDraft] = React.useState("")
  const [saving, setSaving] = React.useState(false)
  const mcpUrl = useMcpUrl()

  React.useEffect(() => {
    let cancelled = false
    call(api.GET("/api/v1/assistant"))
      .then((a) => {
        if (cancelled) return
        setSaved(a.name)
        setDraft(a.name)
      })
      .catch((e: Error) => toast.error(e.message))
    return () => {
      cancelled = true
    }
  }, [])

  const trimmed = draft.trim()
  const invalid = trimmed.length === 0 || trimmed.length > 40

  async function save(e: React.FormEvent) {
    e.preventDefault()
    if (invalid) return
    setSaving(true)
    try {
      const a = await call(
        api.PUT("/api/v1/assistant", { body: { name: trimmed } })
      )
      setSaved(a.name)
      setDraft(a.name)
      toast.success(`Renamed to ${a.name}`)
    } catch (err) {
      toast.error((err as Error).message)
    } finally {
      setSaving(false)
    }
  }

  return (
    <PageBody className="grid lg:grid-cols-2">
      <Alert className="lg:col-span-2">
        <HourglassIcon />
        <AlertTitle>Coming later</AlertTitle>
        <AlertDescription>
          Voice control and asking for music by name are still being built. You
          can choose the assistant&apos;s name now.
        </AlertDescription>
      </Alert>
      <Card>
        <form onSubmit={save} className="flex flex-col gap-6">
          <CardHeader>
            <CardTitle>Name</CardTitle>
            <CardDescription>
              What you call the assistant. With voice control, saying its name
              will wake it, so choose a word nobody says during a service.
            </CardDescription>
          </CardHeader>
          <CardContent>
            {saved === null ? (
              <Skeleton className="h-8 w-full" />
            ) : (
              <FieldGroup>
                <Field data-invalid={invalid || undefined}>
                  <FieldLabel htmlFor="assistant-name">
                    Assistant name
                  </FieldLabel>
                  <Input
                    id="assistant-name"
                    value={draft}
                    maxLength={40}
                    onChange={(e) => setDraft(e.target.value)}
                    aria-invalid={invalid || undefined}
                    className="font-display text-lg md:text-lg"
                  />
                  <FieldDescription>
                    Cecilia, after the patron saint of music, rarely comes up in
                    a service. Short, common words wake it by accident.
                  </FieldDescription>
                </Field>
              </FieldGroup>
            )}
          </CardContent>
          <CardFooter>
            <Button
              type="submit"
              disabled={saving || invalid || trimmed === saved}
            >
              Save name
            </Button>
          </CardFooter>
        </form>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Connect an AI assistant</CardTitle>
          <CardDescription>
            AI assistants such as Claude can run the organ through Cecilia. For
            the technically minded: Cecilia is an MCP server.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="mcp-url">Server address</FieldLabel>
              {mcpUrl ? (
                <CopyField id="mcp-url" value={mcpUrl} label="server address" />
              ) : (
                <Skeleton className="h-8" />
              )}
            </Field>
            <Field>
              <FieldLabel>Claude Code</FieldLabel>
              {mcpUrl ? (
                <CopyBlock
                  value={`claude mcp add --transport http cecilia ${mcpUrl}`}
                  label="Claude Code command"
                />
              ) : (
                <Skeleton className="h-8" />
              )}
              <FieldDescription>
                Other assistants: add an MCP server using the HTTP transport and
                the address above.
              </FieldDescription>
            </Field>
          </FieldGroup>
        </CardContent>
      </Card>

      <Card className="lg:col-span-2">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <MicIcon className="text-muted-foreground" aria-hidden />
            Things you can ask
          </CardTitle>
          <CardDescription>
            {saved ?? "The assistant"} acts on the organ selected as Organ out,
            or on the simulator. If a stop name fits more than one stop, it asks
            which you mean. Asking for songs by name comes with the music
            library.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <ul className="grid gap-2 sm:grid-cols-2">
            {EXAMPLES.map((e) => (
              <li
                key={e}
                className="rounded-lg border bg-muted/40 px-3 py-2 font-display text-base"
              >
                “{e}”
              </li>
            ))}
          </ul>
        </CardContent>
      </Card>
    </PageBody>
  )
}
