import { expect, test } from '@playwright/test'

test('download cards align on desktop and fit their content on mobile', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'userAgentData', {
      value: { platform: 'Linux', mobile: false },
    })
  })
  await page.goto('./')

  const cards = page.locator('#download .platform')
  await expect(cards).toHaveCount(3)
  await expect(cards.last()).toHaveClass(/platform-detected/)

  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 900 })
    const layout = await cards.evaluateAll((elements) =>
      elements.map((card) => {
        const style = getComputedStyle(card)
        const bounds = card.getBoundingClientRect()
        const content =
          card.querySelector('.platform-hint') ?? card.querySelector('.platform-downloads')!
        return {
          system: card.getAttribute('data-platform'),
          top: bounds.top,
          bottom: bounds.bottom,
          left: bounds.left,
          right: bounds.right,
          bottomGap: bounds.bottom - content.getBoundingClientRect().bottom,
          bottomInset: parseFloat(style.paddingBottom) + parseFloat(style.borderBottomWidth),
        }
      }),
    )

    const [first, second, linux] = layout
    expect(linux!.system).toBe('linux')
    expect(linux!.top).toBeGreaterThan(Math.max(first!.bottom, second!.bottom))
    expect(linux!.left).toBeCloseTo(first!.left)
    expect(linux!.right).toBeCloseTo(second!.right)
    if (width === 1280) {
      expect(first!.top).toBeCloseTo(second!.top)
      expect(first!.bottom).toBeCloseTo(second!.bottom)
      expect(linux!.bottomGap).toBeCloseTo(linux!.bottomInset)
    } else {
      expect(first!.bottom).toBeLessThan(second!.top)
      for (const card of layout) expect(card.bottomGap).toBeCloseTo(card.bottomInset)
    }
  }
})

test('roadmap preview, history, and release links work', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.goto('./')
  const preview = page.locator('#roadmap')
  await expect(preview.locator('.roadmap-tag-topic')).toHaveCount(2)
  await expect(preview.locator('.roadmap-tag-release')).toHaveCount(1)
  await preview.locator('.roadmap-link').click()
  await expect(page).toHaveURL(/\/roadmap\/$/)
  const roadmap = page.locator('#roadmap')
  await expect(roadmap.locator('.roadmap-tag-topic')).toHaveCount(4)
  const releases = roadmap.locator('.roadmap-tag-release')
  await expect.poll(() => releases.count()).toBeGreaterThan(1)
  await expect(roadmap.locator('a[href$="/release-notes/unreleased/"]')).toHaveCount(0)
  const release = roadmap.getByRole('link', { name: 'macOS 1.5.0', exact: true })
  await expect(release).toHaveAttribute(
    'href',
    new URL('../docs/release-notes/1.5.0/', page.url()).pathname,
  )
  await release.click()
  await expect(page.getByRole('heading', { level: 1 })).toContainText('1.5.0')
})

for (const width of [390, 1280]) {
  test(`screenshot gallery stays complete and usable at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 })
    await page.emulateMedia({ reducedMotion: 'reduce' })
    await page.goto('./')
    const buttons = page.locator('.preview-options button')
    const screenshots = ['workspace', 'search', 'history', 'comparison', 'settings']
    await expect(buttons).toHaveCount(screenshots.length)
    for (const [index, screenshot] of screenshots.entries()) {
      const button = buttons.nth(index)
      await button.click()
      await expect(button).toHaveAttribute('aria-pressed', 'true')
      const image = page.locator('#product-preview img:visible')
      await expect(image).toHaveCount(1)
      await expect(image).toHaveAttribute('src', new RegExp(`/images/${screenshot}-en\\.webp$`))
      await expect(image).toHaveCSS('object-fit', 'contain')
      await expect
        .poll(() =>
          image.evaluate((element) => {
            const screenshot = element as HTMLImageElement
            return screenshot.complete && screenshot.naturalWidth > 0
          }),
        )
        .toBe(true)
      const bounds = await image.boundingBox()
      expect(bounds!.x).toBeGreaterThanOrEqual(0)
      expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width)
    }
    await expect(page.locator('#features article')).toHaveCount(6)
    await expect(page.locator('.deployment')).toHaveCount(0)
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
      width,
    )
    // Keep local review captures with the test artifacts, outside published assets.
    await page.locator('#features').screenshot({
      path: test.info().outputPath('features.png'),
    })
    await page.locator('.hero').screenshot({
      path: test.info().outputPath('editor-overview.png'),
    })
  })
}
