import { expect, type Page, test } from "@playwright/test"

// The test server points at e2e/fixtures/library: Eventide (bundled with
// Cecilia too), Aberystwyth (Lent), Cranham and Mueller (Christmas), and the
// Makin template.

async function openShared(page: Page) {
  await page.goto("/library/")
  await page.getByRole("tab", { name: "Shared library" }).click()
  await expect(page.getByText("4 pieces")).toBeVisible()
}

/** Choose an occasion, through the Filters sheet on a phone. */
async function chooseOccasion(page: Page, isMobile: boolean, occasion: string) {
  if (isMobile) await page.getByRole("button", { name: /^Filters/ }).click()
  await page
    .locator(isMobile ? "#shared-phone-occasion" : "#shared-occasion")
    .click()
  await page.getByRole("option", { name: occasion }).click()
  if (isMobile)
    await page.getByRole("button", { name: /^Show \d+ pieces/ }).click()
}

test.afterEach(async ({ request }) => {
  await request.delete("/api/v1/library/lib-aberystwyth-parry")
})

test("the shared library filters by occasion and search", async ({
  page,
  isMobile,
}) => {
  await openShared(page)
  await chooseOccasion(page, isMobile, "Christmas")
  await expect(page.getByText("2 of 4 pieces")).toBeVisible()
  await expect(page.getByText("In The Bleak MidWinter")).toBeVisible()
  await expect(page.getByText("Abide with Me")).toHaveCount(0)

  await page.getByRole("button", { name: "Clear filters" }).first().click()
  await page.getByLabel("Search shared music").fill("parry lent")
  await expect(page.getByText("1 of 4 pieces")).toBeVisible()
  await expect(page.getByText("Savior When In Dust To Thee")).toBeVisible()
  await expect(
    page.getByRole("link", { name: "Open Hymnal Project" }).first()
  ).toHaveAttribute("href", /openhymnal\.org/)
})

test("a hymn is added from the shared library", async ({ page, request }) => {
  await openShared(page)
  // Eventide comes with Cecilia, so it's already here.
  const eventide = page
    .getByRole("listitem")
    .filter({ hasText: "Abide with Me" })
  await expect(eventide.getByText("In your library")).toBeVisible()

  const parry = page
    .getByRole("listitem")
    .filter({ hasText: "Savior When In Dust To Thee" })
  await parry.getByRole("button", { name: "Add" }).click()
  await expect(parry.getByText("In your library")).toBeVisible()
  const library = await (await request.get("/api/v1/library")).json()
  const added = library.find(
    (t: { id: string }) => t.id === "lib-aberystwyth-parry"
  )
  expect(added?.info.composer).toContain("Joseph Parry")
})

test("an organ setup is added from the shared library", async ({
  page,
  request,
}) => {
  await page.goto("/organs/")
  await expect(
    page.getByRole("heading", { name: "Shared organ setups" })
  ).toBeVisible()
  await page.getByRole("button", { name: "Add to my organs" }).first().click()
  await expect(page).toHaveURL(/\/organs\/edit\/\?id=/)
  const id = new URL(page.url()).searchParams.get("id")!
  const organ = await (await request.get(`/api/v1/organs/${id}`)).json()
  expect(organ.name).toMatch(/^Makin/)
  await request.post(`/api/v1/organs/${id}/delete`)
})
