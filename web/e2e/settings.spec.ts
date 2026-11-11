import { expect, test } from "@playwright/test"

test("a password locks other browsers until they sign in", async ({ page, browser, request }) => {
  // Set the password from this test's own browser session.
  await page.goto("/setup/settings/")
  await page.getByLabel("Password", { exact: true }).fill("magnificat")
  await page.getByRole("button", { name: "Set password" }).click()
  await expect(page.getByText(/Password set/)).toBeVisible()
  try {
    const other = await browser.newContext()
    const p2 = await other.newPage()
    await p2.goto("/library/")
    await expect(p2.getByRole("button", { name: "Sign in" })).toBeVisible()
    await p2.getByLabel("Password").fill("wrong one")
    await p2.getByRole("button", { name: "Sign in" }).click()
    await expect(p2.getByText("That password isn't right.")).toBeVisible()
    await p2.getByLabel("Password").fill("magnificat")
    await p2.getByRole("button", { name: "Sign in" }).click()
    await expect(p2.getByPlaceholder("Search titles, composers, tunes")).toBeVisible()
    await other.close()
  } finally {
    // Remove it again so other tests aren't locked out.
    await page.goto("/setup/settings/")
    await page.getByRole("button", { name: "Remove password" }).click()
    await expect(page.getByText("Password removed")).toBeVisible()
  }
  expect((await (await request.get("/api/v1/auth")).json()).required).toBe(false)
})
