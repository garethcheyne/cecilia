"use client"

import { CommunityMusic } from "@/components/library/community-music"
import { LibraryView } from "@/components/library/library-view"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

/** Your own library, and music shared by other churches. */
export function LibraryTabs() {
  return (
    <Tabs defaultValue="mine" className="flex flex-col">
      <div className="mx-auto w-full max-w-6xl px-3 pt-3 sm:px-4 sm:pt-4">
        <TabsList>
          <TabsTrigger value="mine">My library</TabsTrigger>
          <TabsTrigger value="shared">Shared library</TabsTrigger>
        </TabsList>
      </div>
      <TabsContent value="mine">
        <LibraryView />
      </TabsContent>
      <TabsContent value="shared">
        <div className="mx-auto w-full min-w-0 max-w-6xl p-3 sm:p-4">
          <CommunityMusic />
        </div>
      </TabsContent>
    </Tabs>
  )
}
