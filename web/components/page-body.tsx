import * as React from "react"

import { cn } from "@/lib/utils"

/**
 * The content area of a page, the same width on every page so moving between
 * them doesn't make the content jump.
 */
export function PageBody({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "mx-auto flex w-full max-w-6xl min-w-0 flex-col gap-4 p-3 sm:p-4",
        className
      )}
      {...props}
    />
  )
}
