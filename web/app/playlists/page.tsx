import type { Metadata } from "next"

import { PageHeader } from "@/components/page-header"
import { PlaylistsIndex } from "@/components/playlists/playlists-index"

export const metadata: Metadata = { title: "Playlists" }

export default function PlaylistsPage() {
  return (
    <>
      <PageHeader title="Playlists" />
      <PlaylistsIndex />
    </>
  )
}
