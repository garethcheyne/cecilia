import { expect, test } from "@playwright/test"

test("right-clicking the piano roll offers edits for what's there", async ({
  page,
}) => {
  await page.goto("/library/edit/?id=sample-old-hundredth")
  const roll = page.getByRole("application", { name: /Piano roll/ })
  await expect(roll).toBeVisible()
  const box = (await roll.boundingBox())!
  // Empty space low in the roll: add a note, add a verse, play from here.
  await roll.click({
    button: "right",
    position: { x: box.width * 0.6, y: box.height - 40 },
  })
  await expect(
    page.getByRole("menuitem", { name: /Start a new verse here/ })
  ).toBeVisible()
  await expect(
    page.getByRole("menuitem", { name: /Play from here/ })
  ).toBeVisible()
  const add = page.getByRole("menuitem", { name: /^Add .* on the/ })
  await expect(add).toBeVisible()
  if (process.env.SHOT) await page.screenshot({ path: process.env.SHOT })
  await add.click()
  await expect(page.getByText("Unsaved")).toBeVisible()
  // The new note is selected; right-clicking it offers note edits.
  await page.keyboard.press("Escape")
})
