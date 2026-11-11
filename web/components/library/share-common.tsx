"use client"

import * as React from "react"
import Link from "next/link"
import { CircleCheckIcon, CircleAlertIcon, UserRoundIcon } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import {
  api,
  call,
  type CommunityAccountStatus,
  type ShareResult,
} from "@/lib/api/client"

/**
 * Whether this Cecilia is signed in to the community, checked each time a
 * share dialog opens. `null` while checking.
 */
export function useCommunityAccount(open: boolean) {
  const [account, setAccount] = React.useState<CommunityAccountStatus | null>(
    null
  )
  const [problem, setProblem] = React.useState<string | null>(null)

  React.useEffect(() => {
    if (!open) return
    let cancelled = false
    call(api.GET("/api/v1/community/account"))
      .then((a) => {
        if (cancelled) return
        setAccount(a)
        setProblem(null)
      })
      .catch((e: Error) => {
        if (!cancelled) setProblem(e.message)
      })
    return () => {
      cancelled = true
    }
  }, [open])

  return { account, problem }
}

/** Shown in place of the form while checking, or when not signed in. */
export function AccountGate({
  account,
  problem,
  children,
}: {
  account: CommunityAccountStatus | null
  problem: string | null
  children: React.ReactNode
}) {
  if (problem) return <ShareProblem message={problem} />
  if (!account) return <Skeleton className="h-32" />
  if (!account.signed_in) {
    return (
      <Alert>
        <UserRoundIcon />
        <AlertTitle>Sign in to share</AlertTitle>
        <AlertDescription className="flex flex-col items-start gap-3">
          <span>
            Sharing with other churches needs a Cecilia Community account. Sign
            in with your email in Settings; it takes a minute.
          </span>
          <Button
            size="sm"
            render={<Link href="/setup/settings/" />}
            nativeButton={false}
          >
            Go to Settings
          </Button>
        </AlertDescription>
      </Alert>
    )
  }
  return <>{children}</>
}

export function ShareProblem({ message }: { message: string }) {
  return (
    <Alert variant="destructive">
      <CircleAlertIcon />
      <AlertTitle>It wasn&apos;t sent</AlertTitle>
      <AlertDescription>{message}</AlertDescription>
    </Alert>
  )
}

/** What happened after sending: waiting for review, or published. */
export function ShareDone({ result }: { result: ShareResult }) {
  const published = result.status === "approved"
  return (
    <Alert>
      <CircleCheckIcon />
      <AlertTitle>{published ? "Published" : "Sent for review"}</AlertTitle>
      <AlertDescription className="flex flex-col gap-2">
        <span>
          {published
            ? "It's in the shared library now, for every church to use."
            : "A moderator will look at it soon. You'll see it in Settings under What you've shared."}
        </span>
        {result.checks.length > 0 && (
          <ul className="list-disc pl-4">
            {result.checks.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        )}
      </AlertDescription>
    </Alert>
  )
}
