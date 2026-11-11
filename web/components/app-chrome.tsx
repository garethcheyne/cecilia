"use client"

import { usePathname } from "next/navigation"

import { AppSidebar } from "@/components/app-sidebar"
import { TransportBar } from "@/components/transport/transport-bar"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"

/** The sidebar and transport bar, except on a bare display window. */
export function AppChrome({ children }: { children: React.ReactNode }) {
  const path = usePathname() ?? ""
  if (path.startsWith("/service/display"))
    return <main className="min-w-0">{children}</main>
  return (
    <SidebarProvider>
      <AppSidebar />
      <SidebarInset className="min-w-0">
        <div className="flex flex-1 flex-col">{children}</div>
        <TransportBar />
      </SidebarInset>
    </SidebarProvider>
  )
}
