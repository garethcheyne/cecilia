import { Suspense } from "react"
import type { Metadata } from "next"

import { TrackEditor } from "@/components/editor/track-editor"
import { PageHeader } from "@/components/page-header"

export const metadata: Metadata = { title: "Edit track" }

export default function EditTrackPage() {
  return (
    <>
      <PageHeader title="Edit track" />
      {/* The track id comes from ?id=, read on the client. */}
      <Suspense>
        <TrackEditor />
      </Suspense>
    </>
  )
}
