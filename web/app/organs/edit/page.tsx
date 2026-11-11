import { Suspense } from "react"
import type { Metadata } from "next"

import { OrganEditor } from "@/components/organs/organ-editor"
import { PageHeader } from "@/components/page-header"

export const metadata: Metadata = { title: "Organ" }

export default function EditOrganPage() {
  return (
    <>
      <PageHeader title="Organ" />
      <Suspense>
        <OrganEditor />
      </Suspense>
    </>
  )
}
