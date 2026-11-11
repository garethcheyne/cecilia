import { Suspense } from "react"
import type { Metadata } from "next"

import { PageHeader } from "@/components/page-header"
import { PlaylistEditor } from "@/components/playlists/playlist-editor"

export const metadata: Metadata = { title: "Playlist" }

export default function EditPlaylistPage() {
  return (
    <>
      <PageHeader title="Playlist" />
      {/* The playlist id comes from ?id=, read on the client. */}
      <Suspense>
        <PlaylistEditor />
      </Suspense>
    </>
  )
}
