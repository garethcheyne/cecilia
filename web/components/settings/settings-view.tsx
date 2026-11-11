"use client"

import * as React from "react"
import { DownloadIcon, LogOutIcon, UploadIcon } from "lucide-react"
import { toast } from "sonner"

import {
  applyTextSize,
  readTextSize,
  TEXT_SIZES,
  type TextSize,
} from "@/components/text-size"
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
import { PageBody } from "@/components/page-body"
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
import { Skeleton } from "@/components/ui/skeleton"
import { Slider } from "@/components/ui/slider"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { api, apiUrl, call, type SettingsBody } from "@/lib/api/client"

function SecondsSlider({
  id,
  label,
  hint,
  value,
  min,
  max,
  onChange,
}: {
  id: string
  label: string
  hint: string
  value: number
  min: number
  max: number
  onChange: (ms: number) => void
}) {
  return (
    <Field>
      <FieldLabel htmlFor={id}>
        {label}: {value / 1000} s
      </FieldLabel>
      <Slider
        id={id}
        min={min}
        max={max}
        step={500}
        value={value}
        onValueChange={(v) => onChange(v as number)}
      />
      <FieldDescription>{hint}</FieldDescription>
    </Field>
  )
}

export function SettingsView() {
  const [settings, setSettings] = React.useState<SettingsBody | null>(null)
  const [textSize, setTextSize] = React.useState<TextSize>(readTextSize)
  const [auth, setAuth] = React.useState<{ required: boolean } | null>(null)
  const [password, setPassword] = React.useState("")
  const [restoreFile, setRestoreFile] = React.useState<File | null>(null)
  const fileInput = React.useRef<HTMLInputElement>(null)
  const dirty = React.useRef(false)

  React.useEffect(() => {
    call(api.GET("/api/v1/settings"))
      .then(setSettings)
      .catch((e: Error) => toast.error(e.message))
    call(api.GET("/api/v1/auth"))
      .then(setAuth)
      .catch(() => {})
  }, [])

  // Save shortly after a change.
  React.useEffect(() => {
    if (!settings || !dirty.current) return
    const t = setTimeout(() => {
      call(api.PUT("/api/v1/settings", { body: settings }))
        .then(() => toast.success("Settings saved"))
        .catch((e: Error) => toast.error(e.message))
      dirty.current = false
    }, 700)
    return () => clearTimeout(t)
  }, [settings])

  const change = (s: SettingsBody) => {
    dirty.current = true
    setSettings(s)
  }

  async function savePassword(newPassword: string | null) {
    try {
      await call(
        api.PUT("/api/v1/auth/password", { body: { password: newPassword } })
      )
      setAuth({ required: newPassword !== null })
      setPassword("")
      toast.success(
        newPassword
          ? "Password set. Other devices will need it."
          : "Password removed"
      )
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  async function restore(file: File) {
    try {
      const res = await fetch(apiUrl("/api/v1/import/backup"), {
        method: "POST",
        body: file,
        credentials: "include",
      })
      const body = await res.json()
      if (!res.ok) throw new Error(body.error ?? "Restore failed")
      toast.success(
        `Restored ${body.tracks} pieces, ${body.organs} organs and ${body.playlists} playlists`
      )
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  return (
    <PageBody className="grid lg:grid-cols-2">
      <Card>
        <CardHeader>
          <CardTitle>Recording</CardTitle>
          <CardDescription>
            How a service is split into separate pieces.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {settings ? (
            <FieldGroup>
              <SecondsSlider
                id="silence"
                label="A piece ends after"
                hint="Silence (with no keys held) that ends one piece; the next note starts another."
                min={2000}
                max={60000}
                value={settings.recording.silence_ms}
                onChange={(v) =>
                  change({
                    ...settings,
                    recording: { ...settings.recording, silence_ms: v },
                  })
                }
              />
              <SecondsSlider
                id="margin-start"
                label="Silence kept before the first note"
                hint="Handy when recording sound alongside."
                min={0}
                max={15000}
                value={settings.recording.margin_start_ms}
                onChange={(v) =>
                  change({
                    ...settings,
                    recording: { ...settings.recording, margin_start_ms: v },
                  })
                }
              />
              <SecondsSlider
                id="margin-end"
                label="Silence kept after the last note"
                hint="Lets the building's echo die away."
                min={0}
                max={15000}
                value={settings.recording.margin_end_ms}
                onChange={(v) =>
                  change({
                    ...settings,
                    recording: { ...settings.recording, margin_end_ms: v },
                  })
                }
              />
            </FieldGroup>
          ) : (
            <Skeleton className="h-48" />
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Playback</CardTitle>
          <CardDescription>
            Fading out, and organs whose stops only toggle.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {settings ? (
            <FieldGroup>
              <SecondsSlider
                id="fade"
                label="Fade-out length"
                hint="How long the Fade button takes to bring the music to silence."
                min={1000}
                max={30000}
                value={settings.playback.fade_ms}
                onChange={(v) =>
                  change({
                    ...settings,
                    playback: { ...settings.playback, fade_ms: v },
                  })
                }
              />
              <Field>
                <FieldLabel htmlFor="settle">
                  Pause after general cancel: {settings.playback.settle_ms} ms
                </FieldLabel>
                <Slider
                  id="settle"
                  min={0}
                  max={3000}
                  step={50}
                  value={settings.playback.settle_ms}
                  onValueChange={(v) =>
                    change({
                      ...settings,
                      playback: {
                        ...settings.playback,
                        settle_ms: v as number,
                      },
                    })
                  }
                />
                <FieldDescription>
                  On organs like the Makin, the stops need a moment to clear
                  before the opening registration is set.
                </FieldDescription>
              </Field>
            </FieldGroup>
          ) : (
            <Skeleton className="h-48" />
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Text size</CardTitle>
          <CardDescription>
            For this device only, so a tablet at the console can use big text.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <ToggleGroup
            value={[textSize]}
            onValueChange={(v) => {
              const size = (v[0] ?? textSize) as TextSize
              setTextSize(size)
              applyTextSize(size)
            }}
            variant="outline"
            aria-label="Text size"
          >
            {TEXT_SIZES.map((t) => (
              <ToggleGroupItem key={t.value} value={t.value}>
                {t.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Backup</CardTitle>
          <CardDescription>
            Everything (music, playlists, organs and settings) in one file. Keep
            a copy somewhere other than the Cecilia box.
          </CardDescription>
        </CardHeader>
        <CardFooter className="flex-wrap gap-2">
          <Button
            render={<a href={apiUrl("/api/v1/export/backup.zip")} download />}
            nativeButton={false}
          >
            <DownloadIcon data-icon="inline-start" />
            Download a backup
          </Button>
          <Button variant="outline" onClick={() => fileInput.current?.click()}>
            <UploadIcon data-icon="inline-start" />
            Restore from a backup
          </Button>
          <input
            ref={fileInput}
            type="file"
            accept=".zip"
            className="sr-only"
            onChange={(e) => {
              const f = e.target.files?.[0]
              e.target.value = ""
              if (f) setRestoreFile(f)
            }}
          />
        </CardFooter>
      </Card>

      <Card className="lg:col-span-2">
        <CardHeader>
          <CardTitle>Password</CardTitle>
          <CardDescription>
            {auth?.required
              ? "A password is needed to use Cecilia from other devices."
              : "Anyone on the network can use Cecilia. Set a password before connecting it to the church Wi-Fi."}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form
            onSubmit={(e) => {
              e.preventDefault()
              if (password.length >= 6) void savePassword(password)
            }}
          >
            <FieldGroup className="max-w-sm">
              <Field>
                <FieldLabel htmlFor="new-password">
                  {auth?.required ? "New password" : "Password"}
                </FieldLabel>
                <Input
                  id="new-password"
                  type="password"
                  autoComplete="new-password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
                <FieldDescription>At least 6 characters.</FieldDescription>
              </Field>
            </FieldGroup>
          </form>
        </CardContent>
        <CardFooter className="flex-wrap gap-2">
          <Button
            disabled={password.length < 6}
            onClick={() => savePassword(password)}
          >
            {auth?.required ? "Change password" : "Set password"}
          </Button>
          {auth?.required && (
            <>
              <Button variant="outline" onClick={() => savePassword(null)}>
                Remove password
              </Button>
              <Button
                variant="ghost"
                onClick={async () => {
                  await api.POST("/api/v1/auth/logout")
                  window.location.reload()
                }}
              >
                <LogOutIcon data-icon="inline-start" />
                Sign out
              </Button>
            </>
          )}
        </CardFooter>
      </Card>

      <AlertDialog
        open={!!restoreFile}
        onOpenChange={(o) => !o && setRestoreFile(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Restore {restoreFile?.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              Pieces, playlists and organs in the backup replace ones with the
              same name here. Anything else is kept. This device&apos;s MIDI
              ports and password don&apos;t change.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (restoreFile) void restore(restoreFile)
                setRestoreFile(null)
              }}
            >
              Restore
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </PageBody>
  )
}
