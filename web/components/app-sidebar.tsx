"use client"

import * as React from "react"
import Link from "next/link"
import { usePathname } from "next/navigation"
import {
  CableIcon,
  MicIcon,
  GaugeIcon,
  LayoutDashboardIcon,
  LibraryIcon,
  ListMusicIcon,
  MonitorPlayIcon,
  PianoIcon,
  type LucideIcon,
} from "lucide-react"

import { RegistraMark } from "@/components/brand/registra-mark"
import { useEngine } from "@/components/engine-provider"
import { Badge } from "@/components/ui/badge"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from "@/components/ui/sidebar"
import { api, call } from "@/lib/api/client"

interface NavItem {
  title: string
  href: string
  icon: LucideIcon
  soon?: boolean
}

const PERFORM: NavItem[] = [
  { title: "Overview", href: "/", icon: LayoutDashboardIcon },
  { title: "Simulator", href: "/simulator", icon: PianoIcon },
  { title: "Console", href: "/console", icon: GaugeIcon, soon: true },
  { title: "Library", href: "/library", icon: LibraryIcon },
  { title: "Playlists", href: "/playlists", icon: ListMusicIcon },
  { title: "Service", href: "/service", icon: MonitorPlayIcon },
]

const SETUP: NavItem[] = [
  { title: "MIDI Setup", href: "/setup/midi", icon: CableIcon },
  { title: "Assistant", href: "/setup/assistant", icon: MicIcon },
]

function NavGroup({ label, items }: { label: string; items: NavItem[] }) {
  // trailingSlash is on, so normalise "/setup/midi/" to "/setup/midi".
  const pathname = usePathname().replace(/(.)\/$/, "$1")
  const active = (href: string) => pathname === href || (href !== "/" && pathname.startsWith(`${href}/`))
  return (
    <SidebarGroup>
      <SidebarGroupLabel>{label}</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>
          {items.map((item) => (
            <SidebarMenuItem key={item.href}>
              {item.soon ? (
                <SidebarMenuButton disabled tooltip={`${item.title} (coming soon)`}>
                  <item.icon />
                  <span>{item.title}</span>
                </SidebarMenuButton>
              ) : (
                <SidebarMenuButton
                  isActive={active(item.href)}
                  tooltip={item.title}
                  render={<Link href={item.href} />}
                >
                  <item.icon />
                  <span>{item.title}</span>
                </SidebarMenuButton>
              )}
              {item.soon && <SidebarMenuBadge>Soon</SidebarMenuBadge>}
            </SidebarMenuItem>
          ))}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  )
}

function ServerStatus() {
  const { connected } = useEngine()
  const [version, setVersion] = React.useState<string | null>(null)

  React.useEffect(() => {
    if (!connected) return
    call(api.GET("/api/v1/health"))
      .then((h) => setVersion(h.version))
      .catch(() => setVersion(null))
  }, [connected])

  return (
    <div className="flex items-center justify-between gap-2 px-2 text-xs text-sidebar-foreground/70 group-data-[collapsible=icon]:hidden">
      <span>Server{version && connected ? ` v${version}` : ""}</span>
      <Badge variant={connected ? "secondary" : "destructive"}>
        {connected ? "Connected" : "Offline"}
      </Badge>
    </div>
  )
}

export function AppSidebar() {
  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" render={<Link href="/" />}>
              <div className="flex aspect-square size-9 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-sm [--mark-cutout:var(--primary)]">
                <RegistraMark className="size-7" />
              </div>
              <div className="flex flex-col gap-0.5 leading-none">
                <span className="font-display text-base font-semibold">Registra</span>
                <span className="text-xs text-sidebar-foreground/70">Organ recorder</span>
              </div>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <NavGroup label="Perform" items={PERFORM} />
        <NavGroup label="Setup" items={SETUP} />
      </SidebarContent>
      <SidebarFooter>
        <ServerStatus />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}
