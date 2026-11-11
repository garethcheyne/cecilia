import { expect, test } from "@playwright/test"

let playlistId = ""

test.beforeEach(async ({ request }) => {
  // A two-hymn service, loaded and waiting.
  const p = await (
    await request.post("/api/v1/playlists", {
      data: { name: `Test service ${Date.now()}` },
    })
  ).json()
  playlistId = p.id
  p.rows = [
    { type: "track", track_id: "sample-old-hundredth" },
    { type: "track", track_id: "sample-eventide" },
  ]
  expect(
    (await request.put(`/api/v1/playlists/${playlistId}`, { data: p })).ok()
  ).toBe(true)
  expect(
    (
      await request.post("/api/v1/service/start", {
        data: { playlist_id: playlistId },
      })
    ).ok()
  ).toBe(true)
})

test.afterEach(async ({ request }) => {
  await request.post("/api/v1/player/stop")
  await request.post("/api/v1/service/end")
  await request.delete(`/api/v1/playlists/${playlistId}`)
})

test("double-clicking a piece in the service plays it at once", async ({
  page,
  request,
}) => {
  await page.goto("/service/")
  const eventide = page
    .getByText("Abide with Me, Fast Falls the Eventide")
    .first()
  await expect(eventide).toBeVisible()
  await eventide.dblclick()
  await expect
    .poll(async () => {
      const p = await (await request.get("/api/v1/player")).json()
      return `${p.track_id} ${p.state}`
    })
    .toBe("sample-eventide playing")
})

test("a second screen shows just the chosen panel", async ({ page }) => {
  await page.goto("/service/display/?panels=now")
  await expect(
    page.getByText("All People That on Earth Do Dwell").first()
  ).toBeVisible()
  await expect(page.getByRole("button", { name: /full screen/i })).toBeVisible()
  // No menus or transport on a display.
  await expect(page.getByRole("link", { name: "Library" })).toHaveCount(0)
})

test("the displays dialog sets up the screens", async ({ page, isMobile }) => {
  test.skip(isMobile, "Second screens are set up from a computer")
  await page.goto("/service/")
  await page.getByRole("button", { name: "Displays" }).click()
  const dialog = page.getByRole("dialog")
  await expect(dialog.getByText("This screen", { exact: true })).toBeVisible()
  await expect(dialog.getByText("Second screen", { exact: true })).toBeVisible()
  await expect(
    dialog.getByRole("button", {
      name: /Open (in a new window|on second screen)/,
    })
  ).toBeVisible()
})
