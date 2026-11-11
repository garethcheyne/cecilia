import { Suspense } from "react"
import type { Metadata } from "next"

import { ServiceDisplay } from "@/components/service/service-display"

export const metadata: Metadata = { title: "Service display" }

/** A second screen for the service: just the chosen panels, no menus. */
export default function ServiceDisplayPage() {
  return (
    <Suspense>
      <ServiceDisplay />
    </Suspense>
  )
}
