"use client"

import { UserRoundIcon } from "lucide-react"
import { toast } from "sonner"

import { useOrganists } from "@/components/settings/use-organists"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { cn } from "@/lib/utils"

const NOBODY = "nobody"

/**
 * Who's playing now: new recordings are credited to them. Shown only once
 * there's at least one organist (added in Settings).
 */
export function OrganistPicker({
  className,
  full = false,
}: {
  className?: string
  full?: boolean
}) {
  const { data, save } = useOrganists()
  if (!data || data.organists.length === 0) return null

  const items = [
    { value: NOBODY, label: "No organist" },
    ...data.organists.map((o) => ({ value: o.id, label: o.name })),
  ]

  async function choose(value: string) {
    if (!data) return
    try {
      await save({ ...data, current: value === NOBODY ? null : value })
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  return (
    <Select
      value={data.current ?? NOBODY}
      onValueChange={(v) => v && void choose(String(v))}
      items={items}
    >
      <SelectTrigger
        size={full ? "default" : "sm"}
        aria-label="Who's playing"
        title="Who's playing: new recordings are credited to them"
        className={cn(full ? "w-full" : "w-auto max-w-44", className)}
      >
        <UserRoundIcon />
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          {items.map((i) => (
            <SelectItem key={i.value} value={i.value}>
              {i.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  )
}
