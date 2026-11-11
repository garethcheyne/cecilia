"use client"

import * as React from "react"

import { useEngine } from "@/components/engine-provider"

/**
 * Collect MIDI arriving from the organ while `active`. `messages` restarts
 * empty each time capture is switched on.
 */
export function useMidiCapture(active: boolean) {
  const { onMidi } = useEngine()
  const [messages, setMessages] = React.useState<number[][]>([])
  const [wasActive, setWasActive] = React.useState(active)
  if (active !== wasActive) {
    setWasActive(active)
    if (active) setMessages([])
  }
  React.useEffect(() => {
    if (!active) return
    return onMidi((e) => {
      if (e.direction === "in") setMessages((m) => [...m, e.bytes])
    })
  }, [active, onMidi])
  return messages
}
