import type { Metadata } from "next"

import { LibraryView } from "@/components/library/library-view"
import { PageHeader } from "@/components/page-header"

export const metadata: Metadata = { title: "Library" }

export default function LibraryPage() {
  return (
    <>
      <PageHeader title="Library" />
      <LibraryView />
    </>
  )
}
