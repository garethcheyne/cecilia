"use client"

import { CommunityMusic } from "@/components/library/community-music"
import { LibraryView } from "@/components/library/library-view"
import { PageBody } from "@/components/page-body"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

/** Your own library, and music shared by other churches. */
export function LibraryTabs() {
  return (
    <Tabs defaultValue="mine" className="flex flex-col">
      <PageBody className="items-start pb-0 sm:pb-0">
        <TabsList>
          <TabsTrigger value="mine">My library</TabsTrigger>
          <TabsTrigger value="shared">Shared library</TabsTrigger>
        </TabsList>
      </PageBody>
      <TabsContent value="mine">
        <LibraryView />
      </TabsContent>
      <TabsContent value="shared">
        <PageBody>
          <CommunityMusic />
        </PageBody>
      </TabsContent>
    </Tabs>
  )
}
