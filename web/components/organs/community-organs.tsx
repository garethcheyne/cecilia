"use client"

import * as React from "react"
import { useRouter } from "next/navigation"
import { DownloadIcon } from "lucide-react"
import { toast } from "sonner"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { api, call, type OrganEntry } from "@/lib/api/client"

const CONFIDENCE: Record<string, { label: string; hint: string }> = {
  template: {
    label: "Template",
    hint: "Worked out from a manual; check the stops on your organ",
  },
  learned: { label: "Learned", hint: "Every stop captured on a real organ" },
  verified: { label: "Verified", hint: "Learned, and tested with playback" },
}

/**
 * Organ setups shared by other churches. Ones that are already built in
 * (same id) are left out, so the same organ isn't offered twice.
 */
export function CommunityOrgans({
  builtinIds,
}: {
  builtinIds: string[] | null
}) {
  const router = useRouter()
  const [organs, setOrgans] = React.useState<OrganEntry[] | null>(null)
  const [problem, setProblem] = React.useState<string | null>(null)

  React.useEffect(() => {
    call(api.GET("/api/v1/community"))
      .then((c) => setOrgans(c.organs))
      .catch((e: Error) => setProblem(e.message))
  }, [])

  const shown =
    organs && builtinIds
      ? organs.filter((o) => !builtinIds.includes(o.id))
      : null

  async function add(o: OrganEntry) {
    try {
      const added = await call(
        api.POST("/api/v1/community/organs/{id}/add", {
          params: { path: { id: o.id } },
        })
      )
      toast.success(`Added ${added.name}. Check it, then press Use this organ.`)
      router.push(`/organs/edit/?id=${encodeURIComponent(added.id)}`)
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  return (
    <section className="flex flex-col gap-3" aria-labelledby="shared-organs">
      <h2 id="shared-organs" className="font-display text-lg font-semibold">
        Shared organ setups
      </h2>
      <p className="text-sm text-muted-foreground">
        Set up by other churches. If yours is the same model, add it and you may
        not need to learn every stop.
      </p>
      {problem ? (
        <p className="text-sm text-muted-foreground">{problem}</p>
      ) : shown === null ? null : shown.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {organs && organs.length > 0
            ? "Nothing shared yet beyond the organs above."
            : "None shared yet."}
        </p>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2">
          {shown.map((o) => {
            const c = CONFIDENCE[o.meta.confidence]
            return (
              <Card key={o.id} size="sm">
                <CardHeader>
                  <CardTitle>
                    {o.meta.maker} {o.meta.model}
                  </CardTitle>
                  <CardDescription>
                    {o.divisions} keyboards · {o.stops} stops
                    {o.meta.years ? ` · ${o.meta.years}` : ""}
                  </CardDescription>
                  <CardAction>
                    <Badge
                      variant={
                        o.meta.confidence === "template"
                          ? "outline"
                          : "secondary"
                      }
                      title={c?.hint}
                    >
                      {c?.label ?? o.meta.confidence}
                    </Badge>
                  </CardAction>
                </CardHeader>
                <CardFooter>
                  <Button size="sm" variant="outline" onClick={() => add(o)}>
                    <DownloadIcon data-icon="inline-start" />
                    Add to my organs
                  </Button>
                </CardFooter>
              </Card>
            )
          })}
        </div>
      )}
    </section>
  )
}
