import { expect, test } from "@playwright/test"

test.beforeEach(async ({ request }) => {
  await request.put("/api/v1/midi/config", { data: { inputs: ["Cecilia Simulator"], output: "Cecilia Simulator" } })
  await request.put("/api/v1/simulator/organ", { data: { organ_id: "makin-2m" } })
  await request.post("/api/v1/player/stop")
})

test("the library lists the sample hymns and plays one", async ({ page }) => {
  await page.goto("/library/")
  await page.getByPlaceholder("Search titles, composers, tunes").fill("eventide")
  const play = page.getByRole("button", { name: /Play Abide with Me/ }).first()
  await play.click()
  await expect(page.getByRole("region", { name: "Transport" }).getByText(/Abide with Me/).filter({ visible: true }).first()).toBeVisible()
  await page.request.post("/api/v1/player/stop")
})

test("a stop is learned from the organ", async ({ page, request }) => {
  const copy = await (await request.post("/api/v1/organs/copy", { data: { from: "makin-2m", name: `Test organ ${Date.now()}` } })).json()
  await page.goto(`/organs/edit/?id=${copy.id}`)
  const row = page.getByText(/^Great Open Diapason/).locator("..")
  await row.getByRole("button", { name: "Learn", exact: true }).click()
  await expect(page.getByText("Pull it out now")).toBeVisible()
  // The simulator plays the part of the console: pull the stop, push it in.
  await request.post("/api/v1/simulator/act", { data: { type: "stop", stop: "gt-opendiap", on: true } })
  await expect(page.getByText("Now push it in")).toBeVisible()
  await request.post("/api/v1/simulator/act", { data: { type: "stop", stop: "gt-opendiap", on: false } })
  await expect(page.getByText("Toggle CB 12").first()).toBeVisible()
  await page.getByRole("button", { name: "Done" }).click()
  await page.getByRole("button", { name: "Save" }).click()
  await expect(page.getByText("Organ saved")).toBeVisible()
  const saved = await (await request.get(`/api/v1/organs/${copy.id}`)).json()
  const od = saved.stops.find((s: { id: string }) => s.id === "gt-opendiap")
  expect(od.midi[0]).toEqual({ type: "toggle", msg: "CB 12" })
})
