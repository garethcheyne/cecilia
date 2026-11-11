import type { Metadata } from "next"

import { LibraryTabs } from "@/components/library/library-tabs"
import { PageHeader } from "@/components/page-header"

export const metadata: Metadata = { title: "Library" }

export default function LibraryPage() {
  return (
    <>
      <PageHeader title="Library" />
      <LibraryTabs />
    </>
  )
}
