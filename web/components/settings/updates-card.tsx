"use client"

import * as React from "react"
import {
  ArchiveIcon,
  CircleCheckIcon,
  DownloadIcon,
  RefreshCwIcon,
  TriangleAlertIcon,
  Undo2Icon,
} from "lucide-react"
import { toast } from "sonner"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import {
  api,
  call,
  type UpdateSettings,
  type UpdateStatus,
} from "@/lib/api/client"

const when = (ms: number) =>
  new Date(ms).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  })

const size = (bytes: number) =>
  bytes > 1_000_000
    ? `${(bytes / 1_000_000).toFixed(1)} MB`
    : `${Math.max(1, Math.round(bytes / 1000))} KB`

/** Cecilia's version, what's new, and how it keeps itself up to date. */
export function UpdatesCard() {
  const [status, setStatus] = React.useState<UpdateStatus | null>(null)
  const [busy, setBusy] = React.useState<null | "check" | "install" | "back">(
    null
  )
  const [confirmBack, setConfirmBack] = React.useState(false)

  React.useEffect(() => {
    call(api.GET("/api/v1/updates"))
      .then(setStatus)
      .catch(() => {})
  }, [])

  async function run(
    kind: "check" | "install" | "back",
    action: () => Promise<UpdateStatus>
  ) {
    setBusy(kind)
    try {
      setStatus(await action())
    } catch (e) {
      toast.error((e as Error).message)
      call(api.GET("/api/v1/updates"))
        .then(setStatus)
        .catch(() => {})
    } finally {
      setBusy(null)
    }
  }

  async function saveSettings(next: UpdateSettings) {
    try {
      setStatus(await call(api.PUT("/api/v1/updates/settings", { body: next })))
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  if (!status) {
    return (
      <Card className="lg:col-span-2">
        <CardHeader>
          <CardTitle>Updates</CardTitle>
        </CardHeader>
        <CardContent>
          <Skeleton className="h-24" />
        </CardContent>
      </Card>
    )
  }

  const restarting =
    status.phase === "installing" || status.phase === "restarting"
  const latest = status.latest

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>Updates</CardTitle>
        <CardDescription>
          You&apos;re on Cecilia {status.current}. It keeps itself up to date,
          and never updates while music is playing, recording or a service is
          running.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-6">
        {status.last_result && (
          <Alert>
            <CircleCheckIcon />
            <AlertTitle>{status.last_result}</AlertTitle>
          </Alert>
        )}
        {status.phase === "failed" && status.error && (
          <Alert variant="destructive">
            <TriangleAlertIcon />
            <AlertTitle>The last update didn&apos;t work</AlertTitle>
            <AlertDescription>
              {status.error} Nothing was changed, and your music is safe.
            </AlertDescription>
          </Alert>
        )}

        {restarting ? (
          <Alert>
            <Spinner />
            <AlertTitle>
              Updating to {latest?.version ?? "a new version"}
            </AlertTitle>
            <AlertDescription>
              Cecilia will restart in a moment. This page reloads by itself when
              it&apos;s back.
            </AlertDescription>
          </Alert>
        ) : status.available && latest ? (
          <section className="flex flex-col gap-3 rounded-lg border p-4">
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-display text-lg font-semibold">
                Version {latest.version} is available
              </span>
              {latest.published && (
                <span className="text-sm text-muted-foreground">
                  released {when(Date.parse(latest.published))}
                </span>
              )}
            </div>
            {latest.notes && (
              <div className="max-h-56 overflow-y-auto rounded-md bg-muted/50 p-3 text-sm whitespace-pre-wrap">
                {latest.notes}
              </div>
            )}
            <div className="flex flex-wrap items-center gap-3">
              <Button
                onClick={() =>
                  run("install", () =>
                    call(api.POST("/api/v1/updates/install"))
                  )
                }
                disabled={!!busy || !!status.blocked}
              >
                {busy === "install" ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <DownloadIcon data-icon="inline-start" />
                )}
                Update now
              </Button>
              <span className="text-sm text-muted-foreground">
                {status.blocked ??
                  "Your music, playlists and organs are backed up first."}
              </span>
            </div>
          </section>
        ) : (
          <div className="flex flex-wrap items-center gap-3">
            <CircleCheckIcon className="size-5 text-primary" aria-hidden />
            <span>
              {latest || status.last_checked
                ? "Cecilia is up to date."
                : "Cecilia hasn't checked for updates yet."}
            </span>
            {status.last_checked && (
              <span className="text-sm text-muted-foreground">
                Checked {when(status.last_checked)}
              </span>
            )}
          </div>
        )}

        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            onClick={() =>
              run("check", () => call(api.POST("/api/v1/updates/check")))
            }
            disabled={!!busy || restarting}
          >
            {busy === "check" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <RefreshCwIcon data-icon="inline-start" />
            )}
            Check now
          </Button>
          {status.previous && (
            <Button
              variant="outline"
              onClick={() => setConfirmBack(true)}
              disabled={!!busy || restarting || !!status.blocked}
            >
              <Undo2Icon data-icon="inline-start" />
              Go back to {status.previous}
            </Button>
          )}
        </div>

        {status.kind === "development" ? (
          <p className="text-sm text-muted-foreground">
            This is a development build, so it&apos;s updated with git rather
            than by itself.
          </p>
        ) : (
          <FieldGroup className="max-w-2xl">
            <Field orientation="horizontal">
              <Switch
                id="updates-auto"
                checked={status.settings.auto_install}
                onCheckedChange={(v) =>
                  saveSettings({ ...status.settings, auto_install: v })
                }
              />
              <div className="flex flex-col gap-0.5">
                <FieldLabel htmlFor="updates-auto">
                  Install updates by itself overnight
                </FieldLabel>
                <FieldDescription>
                  Between 2 and 5 am, only when nothing is playing or recording,
                  and after a backup.
                </FieldDescription>
              </div>
            </Field>
            <Field>
              <FieldLabel>Which versions</FieldLabel>
              <ToggleGroup
                value={[status.settings.channel]}
                onValueChange={(v) =>
                  v[0] &&
                  saveSettings({
                    ...status.settings,
                    channel: v[0] as UpdateSettings["channel"],
                  })
                }
                variant="outline"
                aria-label="Which versions"
              >
                <ToggleGroupItem value="stable">Stable</ToggleGroupItem>
                <ToggleGroupItem value="preview">Preview</ToggleGroupItem>
              </ToggleGroup>
              <FieldDescription>
                Stable versions are tested. Preview versions arrive sooner, for
                people happy to try new things first.
              </FieldDescription>
            </Field>
          </FieldGroup>
        )}

        {status.backups.length > 0 && (
          <section className="flex flex-col gap-2">
            <h3 className="text-sm font-medium">
              Backups taken before updates
            </h3>
            <ul className="flex flex-col gap-1 text-sm text-muted-foreground">
              {status.backups.map((b) => (
                <li key={b.file} className="flex items-center gap-2">
                  <ArchiveIcon className="size-4 shrink-0" aria-hidden />
                  <span className="truncate">{when(b.created)}</span>
                  <Badge variant="outline">{size(b.bytes)}</Badge>
                </li>
              ))}
            </ul>
            <p className="text-xs text-muted-foreground">
              Kept in Cecilia&apos;s data folder; the newest three are kept.
              Restore one from Backup above if you ever need to.
            </p>
          </section>
        )}
      </CardContent>

      <AlertDialog open={confirmBack} onOpenChange={setConfirmBack}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Go back to {status.previous}?</AlertDialogTitle>
            <AlertDialogDescription>
              Cecilia restarts on the version that was running before the last
              update. Your music, playlists and organs stay as they are.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() =>
                run("back", () => call(api.POST("/api/v1/updates/rollback")))
              }
            >
              Go back
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  )
}
