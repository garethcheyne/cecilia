"use client"

import * as React from "react"

import { api, call, type OrganistsBody } from "@/lib/api/client"

/** Fired in this window when the organists change, so every view stays in step. */
const CHANGED = "cecilia-organists-changed"

/** The organists and who's playing now, shared by Settings and the transport. */
export function useOrganists() {
  const [data, setData] = React.useState<OrganistsBody | null>(null)
  const [problem, setProblem] = React.useState<string | null>(null)

  React.useEffect(() => {
    let live = true
    const load = () =>
      call(api.GET("/api/v1/organists"))
        .then((d) => {
          if (live) {
            setData(d)
            setProblem(null)
          }
        })
        .catch((e: Error) => live && setProblem(e.message))
    void load()
    const onChange = (e: Event) => {
      const next = (e as CustomEvent<OrganistsBody>).detail
      if (next) setData(next)
      else void load()
    }
    window.addEventListener(CHANGED, onChange)
    return () => {
      live = false
      window.removeEventListener(CHANGED, onChange)
    }
  }, [])

  const save = React.useCallback(async (body: OrganistsBody) => {
    const saved = await call(api.PUT("/api/v1/organists", { body }))
    setData(saved)
    window.dispatchEvent(new CustomEvent(CHANGED, { detail: saved }))
    return saved
  }, [])

  return { data, problem, save }
}
