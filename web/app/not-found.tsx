import Link from "next/link"
import { CompassIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"

export default function NotFound() {
  return (
    <div className="mx-auto flex w-full max-w-6xl flex-1 items-center p-3 sm:p-4">
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <CompassIcon />
          </EmptyMedia>
          <EmptyTitle>This page isn&apos;t here</EmptyTitle>
          <EmptyDescription>
            The link may be old, or the address mistyped.
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button render={<Link href="/" />} nativeButton={false}>
            Go to Overview
          </Button>
        </EmptyContent>
      </Empty>
    </div>
  )
}
