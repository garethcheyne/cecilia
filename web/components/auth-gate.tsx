"use client"

import * as React from "react"
import { LockIcon } from "lucide-react"

import { CeciliaMark } from "@/components/brand/cecilia-mark"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { api } from "@/lib/api/client"

/** Shows a sign-in screen when the server has a password and this browser isn't signed in. */
export function AuthGate({ children }: { children: React.ReactNode }) {
  const [state, setState] = React.useState<"checking" | "open" | "locked">("checking")
  const [password, setPassword] = React.useState("")
  const [problem, setProblem] = React.useState<string | null>(null)
  const [busy, setBusy] = React.useState(false)

  React.useEffect(() => {
    api
      .GET("/api/v1/auth")
      .then(({ data }) => setState(data && data.required && !data.signed_in ? "locked" : "open"))
      // Server unreachable: let the app show its own "can't reach" state.
      .catch(() => setState("open"))
  }, [])

  async function signIn(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setProblem(null)
    const { error } = await api.POST("/api/v1/auth/login", { body: { password } })
    setBusy(false)
    if (error) setProblem((error as { error?: string }).error ?? "That didn't work.")
    else window.location.reload()
  }

  if (state === "open") return <>{children}</>
  if (state === "checking") return null
  return (
    <div className="flex min-h-svh items-center justify-center bg-background p-4">
      <Card className="w-full max-w-sm">
        <form onSubmit={signIn} className="flex flex-col gap-6">
          <CardHeader className="items-center text-center">
            <div className="mx-auto flex size-12 items-center justify-center rounded-2xl bg-primary text-primary-foreground [--mark-cutout:var(--primary)]">
              <CeciliaMark className="size-8" />
            </div>
            <CardTitle className="text-xl">Cecilia</CardTitle>
            <CardDescription>Enter the password to use the organ.</CardDescription>
          </CardHeader>
          <CardContent>
            <FieldGroup>
              <Field data-invalid={!!problem || undefined}>
                <FieldLabel htmlFor="password">Password</FieldLabel>
                <Input
                  id="password"
                  type="password"
                  autoComplete="current-password"
                  autoFocus
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  aria-invalid={!!problem || undefined}
                />
                {problem && <FieldError>{problem}</FieldError>}
              </Field>
            </FieldGroup>
          </CardContent>
          <CardFooter>
            <Button type="submit" className="w-full" disabled={busy || !password}>
              <LockIcon data-icon="inline-start" />
              Sign in
            </Button>
          </CardFooter>
        </form>
      </Card>
    </div>
  )
}
