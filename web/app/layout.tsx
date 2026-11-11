import type { Metadata } from "next"
import { Geist, Geist_Mono, Instrument_Sans } from "next/font/google"

import "./globals.css"
import { AppSidebar } from "@/components/app-sidebar"
import { EngineProvider } from "@/components/engine-provider"
import { ThemeProvider } from "@/components/theme-provider"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"

const geist = Geist({ subsets: ["latin"], variable: "--font-sans" })
const fontMono = Geist_Mono({ subsets: ["latin"], variable: "--font-mono" })
// Headings and stop labels. Its width axis lets long stop names sit condensed on a knob.
const display = Instrument_Sans({ subsets: ["latin"], axes: ["wdth"], variable: "--font-display" })

export const metadata: Metadata = {
  title: { default: "Registra", template: "%s · Registra" },
  description: "Record, edit and play back organ performances over MIDI.",
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html
      lang="en"
      suppressHydrationWarning
      className={cn("antialiased", fontMono.variable, display.variable, "font-sans", geist.variable)}
    >
      <body>
        <ThemeProvider>
          <TooltipProvider>
            <EngineProvider>
              <SidebarProvider>
                <AppSidebar />
                <SidebarInset className="min-w-0">{children}</SidebarInset>
              </SidebarProvider>
              <Toaster />
            </EngineProvider>
          </TooltipProvider>
        </ThemeProvider>
      </body>
    </html>
  )
}
