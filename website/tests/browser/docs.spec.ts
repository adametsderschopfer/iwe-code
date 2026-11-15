import { expect, test } from '@playwright/test'

for (const width of [390, 1280]) {
  test(`CLI explanations appear below their commands at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 850 })
    await page.goto('./docs/cli/')
    for (const heading of ['Commands', 'Deployment commands']) {
      const section = page.locator('.docs-section').filter({
        has: page.getByRole('heading', { name: heading, exact: true }),
      })
      const pairs = await section.locator('dt').evaluateAll((terms) =>
        terms.map((term) => {
          const description = term.nextElementSibling
          const command = term.getBoundingClientRect()
          const explanation = description?.getBoundingClientRect()
          return {
            startsBelow: explanation && explanation.top >= command.bottom,
            sameColumn: explanation && explanation.left === command.left,
          }
        }),
      )
      expect(pairs.length).toBeGreaterThan(0)
      expect(pairs.every(({ startsBelow, sameColumn }) => startsBelow && sameColumn)).toBe(true)
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
      width,
    )
  })
}
