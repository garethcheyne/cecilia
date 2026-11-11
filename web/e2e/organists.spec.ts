import { expect, test } from "@playwright/test"

test.afterEach(async ({ request }) => {
  await request.put("/api/v1/organists", {
    data: { organists: [], current: null },
  })
})

test("an organist is added and chosen as who's playing", async ({
  page,
  request,
}) => {
  await page.goto("/setup/settings/")
  await page.getByRole("button", { name: /Add an organist/ }).click()
  await page.getByLabel(/^Name/).last().fill("Margaret Smith")
  await page.getByRole("button", { name: /Save organists/ }).click()
  await expect(page.getByText("Organists saved")).toBeVisible()

  const saved = await (await request.get("/api/v1/organists")).json()
  expect(saved.organists.map((o: { name: string }) => o.name)).toEqual([
    "Margaret Smith",
  ])
  expect(saved.organists[0].credit_publicly).toBe(false)

  await request.put("/api/v1/organists", {
    data: { ...saved, current: saved.organists[0].id },
  })
  const now = await (await request.get("/api/v1/organists")).json()
  expect(now.current).toBe(saved.organists[0].id)
})

test("sharing explains that it needs the community service", async ({
  request,
}) => {
  // The test server reads the library from a folder, so there's nothing to sign in to.
  const r = await request.post("/api/v1/community/account/start", {
    data: { email: "a@example.com" },
  })
  expect(r.status()).toBe(503)
  expect((await r.json()).error).toMatch(/Cecilia Community service/)
})
