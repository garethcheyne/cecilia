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

test("drawing a stop on screen draws it in the engine", async ({
  page,
  request,
}) => {
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
  const key = page
    .getByRole("group", { name: "Great keys" })
    .getByRole("button", { name: "C4", exact: true })
  await key.dispatchEvent("pointerdown", { pointerId: 1, buttons: 1 })
  await expect(key).toHaveAttribute("aria-pressed", "true")
  await key.dispatchEvent("pointerup", { pointerId: 1 })
  await expect(key).toHaveAttribute("aria-pressed", "false")
})

test("changes made elsewhere appear live", async ({ page, request }) => {
  await page.goto("/simulator/")
  const trumpet = page.getByRole("button", { name: /Trumpet/ })
  await expect(trumpet).toHaveAttribute("aria-pressed", "false")
  await request.post("/api/v1/simulator/act", {
    data: { type: "stop", stop: "gt-trumpet", on: true },
  })
  await expect(trumpet).toHaveAttribute("aria-pressed", "true")
})

test("pressing a key with no stops drawn draws a soft stop so it's heard", async ({
  page,
  request,
}) => {
  await page.goto("/simulator/")
  const swell = page.getByRole("group", { name: /swell keys/i })
  await swell
    .getByRole("button", { name: "C4" })
    .first()
    .dispatchEvent("pointerdown", { pointerId: 1, buttons: 1 })
  await expect(page.getByText(/so you can hear the keys/)).toBeVisible()
  await expect
    .poll(
      async () =>
        (await (await request.get("/api/v1/simulator")).json()).console.stops
    )
    .toContain("sw-gedackt")
  await swell
    .getByRole("button", { name: "C4" })
    .first()
    .dispatchEvent("pointerup", { pointerId: 1 })
})

test("keys played through a coupler light up on their own keyboard", async ({
  page,
  request,
}) => {
  await page.goto("/simulator/")
  await request.post("/api/v1/simulator/act", {
    data: { type: "stop", stop: "gt-sw", on: true },
  })
  await request.post("/api/v1/simulator/act", {
    data: { type: "note", division: "great", key: 60, velocity: 96 },
  })
  const swellC4 = page
    .getByRole("group", { name: /swell keys/i })
    .getByRole("button", { name: "C4" })
    .first()
  await expect(swellC4).toHaveAttribute("data-coupled", "true")
  await request.post("/api/v1/simulator/act", {
    data: { type: "note", division: "great", key: 60, velocity: 0 },
  })
  await expect(swellC4).not.toHaveAttribute("data-coupled", "true")
})
