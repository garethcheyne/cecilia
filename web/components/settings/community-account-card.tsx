"use client"

import * as React from "react"
import { LogOutIcon, MailIcon, RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { Badge } from "@/components/ui/badge"
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
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Separator } from "@/components/ui/separator"
import { Skeleton } from "@/components/ui/skeleton"
import { api, call, type CommunityAccountStatus } from "@/lib/api/client"

type Shared = {
  id: string
  kind: "organ" | "music"
  item_id: string
  status: "pending" | "approved" | "rejected" | "withdrawn"
  title: string
  checks: string[]
  reason: string
  created: number
  reviewed_at: number | null
}

const STATUS: Record<
  Shared["status"],
  {
    label: string
    variant: "default" | "secondary" | "outline" | "destructive"
  }
> = {
  pending: { label: "Waiting for review", variant: "outline" },
  approved: { label: "Published", variant: "default" },
  rejected: { label: "Not accepted", variant: "destructive" },
  withdrawn: { label: "Withdrawn", variant: "secondary" },
}

/** Sign in to the Cecilia Community, and see what's been shared. */
export function CommunityAccountCard() {
  const [account, setAccount] = React.useState<CommunityAccountStatus | null>(
    null
  )
  const [problem, setProblem] = React.useState<string | null>(null)

  React.useEffect(() => {
    call(api.GET("/api/v1/community/account"))
      .then(setAccount)
      .catch((e: Error) => setProblem(e.message))
  }, [])

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>Cecilia Community</CardTitle>
        <CardDescription>
          Sign in to share organ setups and recordings with other churches.
          Browsing and adding from the shared library doesn&apos;t need an
          account.
        </CardDescription>
      </CardHeader>
      {problem ? (
        <CardContent>
          <p className="text-sm text-muted-foreground">{problem}</p>
        </CardContent>
      ) : !account ? (
        <CardContent>
          <Skeleton className="h-20" />
        </CardContent>
      ) : account.signed_in ? (
        <SignedIn account={account} onChange={setAccount} />
      ) : (
        <SignIn onSignedIn={setAccount} />
      )}
    </Card>
  )
}

function SignIn({
  onSignedIn,
}: {
  onSignedIn: (a: CommunityAccountStatus) => void
}) {
  const [email, setEmail] = React.useState("")
  const [code, setCode] = React.useState("")
  const [sent, setSent] = React.useState<{
    minutes: number
    dev: string | null
  } | null>(null)
  const [busy, setBusy] = React.useState(false)
  const [problem, setProblem] = React.useState<string | null>(null)

  async function send(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setProblem(null)
    try {
      const r = await call(
        api.POST("/api/v1/community/account/start", { body: { email } })
      )
      setSent({ minutes: r.minutes, dev: r.code ?? null })
      if (r.code) setCode(r.code)
    } catch (err) {
      setProblem((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  async function verify(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setProblem(null)
    try {
      const a = await call(
        api.POST("/api/v1/community/account/verify", { body: { email, code } })
      )
      toast.success("Signed in to the Cecilia Community")
      onSignedIn(a)
    } catch (err) {
      setProblem((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <CardContent>
      {!sent ? (
        <form onSubmit={send}>
          <FieldGroup>
            <Field data-invalid={problem ? true : undefined}>
              <FieldLabel htmlFor="community-email">Email</FieldLabel>
              <div className="flex flex-wrap gap-2">
                <Input
                  id="community-email"
                  type="email"
                  autoComplete="email"
                  className="max-w-sm"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="you@example.com"
                  aria-invalid={problem ? true : undefined}
                  required
                />
                <Button type="submit" disabled={busy || !email.trim()}>
                  <MailIcon data-icon="inline-start" />
                  Send code
                </Button>
              </div>
              <FieldDescription>
                We&apos;ll email a six-digit code. There&apos;s no password to
                remember.
              </FieldDescription>
              {problem && (
                <FieldDescription className="text-destructive">
                  {problem}
                </FieldDescription>
              )}
            </Field>
          </FieldGroup>
        </form>
      ) : (
        <form onSubmit={verify}>
          <FieldGroup>
            <Field data-invalid={problem ? true : undefined}>
              <FieldLabel htmlFor="community-code">
                Code from the email
              </FieldLabel>
              <div className="flex flex-wrap gap-2">
                <Input
                  id="community-code"
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  className="max-w-40 tracking-widest"
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                  placeholder="123456"
                  aria-invalid={problem ? true : undefined}
                  required
                />
                <Button
                  type="submit"
                  disabled={busy || code.replace(/\D/g, "").length !== 6}
                >
                  Sign in
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => {
                    setSent(null)
                    setCode("")
                    setProblem(null)
                  }}
                >
                  Use another email
                </Button>
              </div>
              <FieldDescription>
                {sent.dev
                  ? `Development code: ${sent.dev}`
                  : `Sent to ${email}. It lasts ${sent.minutes} minutes; check spam if it doesn't arrive.`}
              </FieldDescription>
              {problem && (
                <FieldDescription className="text-destructive">
                  {problem}
                </FieldDescription>
              )}
            </Field>
          </FieldGroup>
        </form>
      )}
    </CardContent>
  )
}

function SignedIn({
  account,
  onChange,
}: {
  account: CommunityAccountStatus
  onChange: (a: CommunityAccountStatus) => void
}) {
  const [name, setName] = React.useState(account.name ?? "")
  const [church, setChurch] = React.useState(account.church ?? "")
  const [busy, setBusy] = React.useState(false)
  const changed =
    name !== (account.name ?? "") || church !== (account.church ?? "")

  async function saveProfile(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    try {
      onChange(
        await call(
          api.PUT("/api/v1/community/account/profile", {
            body: { name, church },
          })
        )
      )
      toast.success("Saved")
    } catch (err) {
      toast.error((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  async function signOut() {
    try {
      await api.POST("/api/v1/community/account/sign-out")
    } catch {
      // Signing out locally still works when the service can't be reached.
    }
    onChange({
      signed_in: false,
      email: null,
      name: null,
      church: null,
      role: null,
    })
  }

  const role =
    account.role === "trusted"
      ? "Trusted"
      : account.role === "moderator"
        ? "Moderator"
        : null

  return (
    <>
      <CardContent className="flex flex-col gap-6">
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <span>
            Signed in as <span className="font-medium">{account.email}</span>
          </span>
          {role && <Badge variant="secondary">{role}</Badge>}
        </div>
        <form onSubmit={saveProfile}>
          <FieldGroup>
            <div className="grid gap-4 sm:grid-cols-2">
              <Field>
                <FieldLabel htmlFor="community-name">Your name</FieldLabel>
                <Input
                  id="community-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="community-church">Church</FieldLabel>
                <Input
                  id="community-church"
                  value={church}
                  onChange={(e) => setChurch(e.target.value)}
                  placeholder="e.g. St Mary's, Fairview"
                />
              </Field>
            </div>
            <Field orientation="horizontal" className="justify-between">
              <FieldDescription>
                How other churches see you when you share something.
              </FieldDescription>
              <Button
                type="submit"
                variant="outline"
                disabled={!changed || busy}
              >
                Save
              </Button>
            </Field>
          </FieldGroup>
        </form>
        <Separator />
        <SharedList />
      </CardContent>
      <CardFooter className="mt-6">
        <Button variant="outline" onClick={signOut}>
          <LogOutIcon data-icon="inline-start" />
          Sign out
        </Button>
      </CardFooter>
    </>
  )
}

function SharedList() {
  const [items, setItems] = React.useState<Shared[] | null>(null)
  const [problem, setProblem] = React.useState<string | null>(null)
  const [version, setVersion] = React.useState(0)

  React.useEffect(() => {
    call(api.GET("/api/v1/community/shared"))
      .then((r) => {
        const list = (Array.isArray(r) ? r : []) as Shared[]
        setItems([...list].sort((a, b) => b.created - a.created))
        setProblem(null)
      })
      .catch((e: Error) => setProblem(e.message))
  }, [version])

  return (
    <section className="flex flex-col gap-3" aria-labelledby="community-shared">
      <div className="flex items-center justify-between gap-2">
        <h3 id="community-shared" className="font-medium">
          What you&apos;ve shared
        </h3>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setVersion((v) => v + 1)}
        >
          <RefreshCwIcon data-icon="inline-start" />
          Check again
        </Button>
      </div>
      {problem ? (
        <p className="text-sm text-muted-foreground">{problem}</p>
      ) : !items ? (
        <Skeleton className="h-16" />
      ) : items.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyTitle>Nothing shared yet</EmptyTitle>
            <EmptyDescription>
              Share an organ setup from its page in Organs, or a recording from
              the Library.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ul className="flex flex-col divide-y overflow-hidden rounded-xl border">
          {items.map((s) => {
            const st = STATUS[s.status] ?? STATUS.pending
            return (
              <li key={s.id} className="flex flex-col gap-1 px-4 py-3">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className="min-w-0 truncate font-medium">
                    {s.title}
                  </span>
                  <Badge variant={st.variant}>{st.label}</Badge>
                </div>
                <span className="text-xs text-muted-foreground">
                  {s.kind === "organ" ? "Organ setup" : "Music"} ·{" "}
                  {new Date(s.created).toLocaleDateString()}
                </span>
                {s.status === "rejected" && s.reason && (
                  <p className="text-sm text-destructive">{s.reason}</p>
                )}
                {s.checks.length > 0 && (
                  <ul className="list-disc pl-4 text-xs text-muted-foreground">
                    {s.checks.map((c) => (
                      <li key={c}>{c}</li>
                    ))}
                  </ul>
                )}
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}
