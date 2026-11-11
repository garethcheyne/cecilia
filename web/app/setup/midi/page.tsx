import type { Metadata } from "next"

import { MonitorCard } from "@/components/midi/monitor-card"
import { PortsCard } from "@/components/midi/ports-card"
import { TestRecorderCard } from "@/components/midi/test-recorder-card"
import { PageBody } from "@/components/page-body"
import { PageHeader } from "@/components/page-header"

export const metadata: Metadata = { title: "MIDI Setup" }

export default function MidiSetupPage() {
  return (
    <>
      <PageHeader title="MIDI Setup" />
      <PageBody className="grid lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
        <PortsCard />
        <div className="flex min-w-0 flex-col gap-4">
          <TestRecorderCard />
          <MonitorCard />
        </div>
      </PageBody>
    </>
  )
}
