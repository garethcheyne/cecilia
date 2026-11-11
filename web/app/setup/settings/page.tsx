import type { Metadata } from "next"

import { PageHeader } from "@/components/page-header"
import { SettingsView } from "@/components/settings/settings-view"

export const metadata: Metadata = { title: "Settings" }

export default function SettingsPage() {
  return (
    <>
      <PageHeader title="Settings" />
      <SettingsView />
    </>
  )
}
