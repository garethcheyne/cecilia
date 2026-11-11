import { expect, test } from "@playwright/test"

test.beforeEach(async ({ request }) => {
  // Every test starts from the demo organ with all stops in.
  await request.put("/api/v1/simulator/organ", { data: { organ_id: "demo" } })
  await request.post("/api/v1/simulator/reset")
})

test("the overview shows the server is connected", async ({ page }) => {
  await page.goto("/")
  await expect(page.getByText("Connected").first()).toBeVisible()
  await expect(page.getByText("Try the simulator")).toBeVisible()
})

test("drawing a stop on screen draws it in the engine", async ({ page, request }) => {
  await page.goto("/simulator/")
  const stop = page.getByRole("button", { name: /Open Diapason/ })
  await expect(stop).toHaveAttribute("aria-pressed", "false")
  await stop.click()
  await expect(stop).toHaveAttribute("aria-pressed", "true")
  const sim = await (await request.get("/api/v1/simulator")).json()
  expect(sim.console.stops).toContain("gt-opendiap")
})

test("general cancel pushes every stop in", async ({ page }) => {
  await page.goto("/simulator/")
  const principal = page.getByRole("button", { name: /^Principal 4/ })
  await principal.click()
  await expect(principal).toHaveAttribute("aria-pressed", "true")
  await page.getByRole("button", { name: /cancel/i }).click()
  await expect(principal).toHaveAttribute("aria-pressed", "false")
})

test("pressing a key lights it up", async ({ page }) => {
  await page.goto("/simulator/")
  const key = page.getByRole("group", { name: "Great keys" }).getByRole("button", { name: "C4", exact: true })
  await key.dispatchEvent("pointerdown", { pointerId: 1, buttons: 1 })
  await expect(key).toHaveAttribute("aria-pressed", "true")
  await key.dispatchEvent("pointerup", { pointerId: 1 })
  await expect(key).toHaveAttribute("aria-pressed", "false")
})

test("changes made elsewhere appear live", async ({ page, request }) => {
  await page.goto("/simulator/")
  const trumpet = page.getByRole("button", { name: /Trumpet/ })
  await expect(trumpet).toHaveAttribute("aria-pressed", "false")
  await request.post("/api/v1/simulator/act", { data: { type: "stop", stop: "gt-trumpet", on: true } })
  await expect(trumpet).toHaveAttribute("aria-pressed", "true")
})
