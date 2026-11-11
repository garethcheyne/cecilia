import type { Metadata } from "next"

import { PageHeader } from "@/components/page-header"
import { ServiceView } from "@/components/service/service-view"

export const metadata: Metadata = { title: "Service" }

export default function ServicePage() {
  return (
    <>
      <PageHeader title="Service" />
      <ServiceView />
    </>
  )
}
