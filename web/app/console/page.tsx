import type { Metadata } from "next"

import { ConsoleView } from "@/components/console/console-view"

export const metadata: Metadata = { title: "Console" }

export default function ConsolePage() {
  return <ConsoleView />
}
