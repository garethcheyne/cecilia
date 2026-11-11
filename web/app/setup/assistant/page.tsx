import type { Metadata } from "next"

import { AssistantSetup } from "@/components/assistant/assistant-setup"
import { PageHeader } from "@/components/page-header"

export const metadata: Metadata = { title: "Assistant" }

export default function AssistantPage() {
  return (
    <>
      <PageHeader title="Assistant" />
      <AssistantSetup />
    </>
  )
}
