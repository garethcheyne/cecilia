import type { Metadata } from "next"

import { OrgansIndex } from "@/components/organs/organs-index"
import { PageHeader } from "@/components/page-header"

export const metadata: Metadata = { title: "Organs" }

export default function OrgansPage() {
  return (
    <>
      <PageHeader title="Organs" />
      <OrgansIndex />
    </>
  )
}
