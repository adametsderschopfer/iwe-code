import { expect, test, type Locator, type Page } from '@playwright/test'

async function openWebsite(page: Page) {
  await page.goto('./')
  await page.waitForLoadState('networkidle')
}

async function wheelTo(page: Page, target: Locator) {
  const bounds = await target.boundingBox()
  expect(bounds).not.toBeNull()
  await page.mouse.wheel(0, Math.round(bounds!.y - 150))
  await expect.poll(async () => (await target.boundingBox())?.y ?? Infinity).toBeLessThan(200)
}

async function imageIsExposed(image: Locator) {
  return image.evaluate((element) => {
    const imageElement = element as HTMLImageElement
    if (!imageElement.complete || !imageElement.naturalWidth) return false
    const rect = element.getBoundingClientRect()
    const left = Math.max(0, rect.left)
    const top = Math.max(0, rect.top)
    const right = Math.min(innerWidth, rect.right)
    const bottom = Math.min(innerHeight, rect.bottom)
    if (right <= left || bottom <= top) return false
    return document.elementFromPoint((left + right) / 2, (top + bottom) / 2) === element
  })
}

test('gallery tilt, opacity, and progress track the scroll position', async ({ page }) => {
  await openWebsite(page)
  const stage = page.locator('.preview-stage')
  const progress = () =>
    page.locator('.landing').evaluate((element) => {
      const transform = getComputedStyle(element, '::before').transform
      return new DOMMatrixReadOnly(transform).m11
    })
  const tilt = () =>
    stage.evaluate((element) => {
      const transform = getComputedStyle(element).transform
      return Math.abs(new DOMMatrixReadOnly(transform).m23)
    })
  await expect.poll(progress).toBe(0)
  await expect.poll(tilt).toBeGreaterThan(0)
  const initialOpacity = Number(
    await stage.evaluate((element) => getComputedStyle(element).opacity),
  )
  expect(initialOpacity).toBeLessThan(0.65)

  await page.mouse.wheel(0, Math.round((await stage.boundingBox())!.y - 80))
  await expect.poll(tilt).toBeLessThan(0.001)
  await expect(stage).toHaveCSS('opacity', '1')
  await expect.poll(progress).toBeGreaterThan(0)

  await page.mouse.wheel(0, -100_000)
  await expect.poll(progress).toBe(0)
  await expect.poll(tilt).toBeGreaterThan(0)
  await expect
    .poll(async () => Number(await stage.evaluate((element) => getComputedStyle(element).opacity)))
    .toBeCloseTo(initialOpacity)
})

test('wipe image becomes exposed after wheel scrolling from below the fold', async ({ page }) => {
  await openWebsite(page)
  const wipe = page.locator('.reveal-wipe')
  const image = wipe.locator('img')
  await expect(wipe).toHaveAttribute('data-reveal-state', 'pending')
  expect((await wipe.boundingBox())!.y).toBeGreaterThan(800)

  await wheelTo(page, wipe)

  await expect(wipe).toHaveAttribute('data-reveal-state', 'visible')
  await expect.poll(() => imageIsExposed(image)).toBe(true)
})

test('the workspace diagram draws its connection and reveals the rows on mobile', async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 800 })
  await openWebsite(page)
  const diagram = page.locator('.local-diagram')
  const connection = diagram.locator('.diagram-connection')
  const lastRow = diagram.locator('.diagram-row').last()
  await expect(diagram).toHaveAttribute('data-reveal-state', 'pending')
  await expect(connection).toHaveCSS('transform', 'matrix(1, 0, 0, 0, 0, 0)')
  await expect(lastRow).toHaveCSS('opacity', '0')

  await wheelTo(page, diagram)

  await expect(diagram).toHaveAttribute('data-reveal-state', 'visible')
  await expect(connection).toHaveCSS('transform', 'none')
  await expect(lastRow).toHaveCSS('opacity', '1')
  await expect(page.locator('.preview-stage')).toHaveCSS('animation-name', /^preview-fade/)
})

test('Workflow anchor reveals the comparison without manual intervention', async ({ page }) => {
  await openWebsite(page)
  await page.getByRole('link', { name: 'Workflow', exact: true }).click()
  await expect(page).toHaveURL(/#workflow$/)
  const wipe = page.locator('.reveal-wipe')
  await expect(wipe).toHaveAttribute('data-reveal-state', 'visible')
  await expect.poll(() => imageIsExposed(wipe.locator('img'))).toBe(true)
})

test('rise and wipe content remain revealed after scrolling back', async ({ page }) => {
  await openWebsite(page)
  await expect(page.locator('.reveal-rise[data-reveal-state="pending"]').first()).toBeAttached()
  await expect(page.locator('.reveal-wipe[data-reveal-state="pending"]')).toBeAttached()

  for (let step = 0; step < 20; step += 1) {
    const position = await page.evaluate(() => ({
      y: scrollY,
      end: document.documentElement.scrollHeight - innerHeight,
    }))
    if (position.y >= position.end - 1) break
    await page.mouse.wheel(0, 600)
    await expect.poll(() => page.evaluate(() => scrollY)).toBeGreaterThan(position.y)
  }
  await expect(page.locator('.scroll-reveal[data-reveal-state="pending"]')).toHaveCount(0)
  await page.mouse.wheel(0, -100_000)
  await expect.poll(() => page.evaluate(() => scrollY)).toBe(0)
  await expect(page.locator('.scroll-reveal[data-reveal-state="pending"]')).toHaveCount(0)
})

test('reduced motion keeps all content visible without scrolling', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await openWebsite(page)
  await expect(page.locator('.scroll-reveal[data-reveal-state="pending"]')).toHaveCount(0)
  await expect(page.locator('.preview-stage')).toHaveCSS('animation-name', 'none')
  await expect(page.locator('.local-diagram .diagram-row').last()).toHaveCSS('opacity', '1')
  const wipe = page.locator('.reveal-wipe')
  await wheelTo(page, wipe)
  await expect.poll(() => imageIsExposed(wipe.locator('img'))).toBe(true)
})

test('changing to reduced motion reveals pending content immediately', async ({ page }) => {
  await openWebsite(page)
  const diagram = page.locator('.local-diagram')
  await expect(diagram).toHaveAttribute('data-reveal-state', 'pending')

  await page.emulateMedia({ reducedMotion: 'reduce' })

  await expect(page.locator('.scroll-reveal[data-reveal-state="pending"]')).toHaveCount(0)
  await expect(diagram.locator('.diagram-row').last()).toHaveCSS('opacity', '1')
})

test('keyboard navigation exposes a pending download card immediately', async ({ page }) => {
  await openWebsite(page)
  await expect(page.locator('.platform[data-reveal-state="pending"]').first()).toBeAttached()
  await page.locator('.hero-actions a').last().focus()
  for (let step = 0; step < 12; step += 1) {
    await page.keyboard.press('Tab')
    if (await page.locator('.platform a:focus').count()) break
  }
  const focusedCard = page.locator('.platform:has(a:focus)')
  await expect(focusedCard).toHaveCount(1)
  await expect(focusedCard).toHaveAttribute('data-reveal-state', 'visible')
  await expect(focusedCard).toHaveClass(/reveal-immediate/)
  await expect(focusedCard).toHaveCSS('opacity', '1')
})

test.describe('without JavaScript', () => {
  test.use({ javaScriptEnabled: false })

  test('server-rendered content and comparison remain visible', async ({ page }) => {
    await openWebsite(page)
    await expect(page.locator('.scroll-reveal[data-reveal-state="pending"]')).toHaveCount(0)
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
    const wipe = page.locator('.reveal-wipe')
    await wheelTo(page, wipe)
    await expect.poll(() => imageIsExposed(wipe.locator('img'))).toBe(true)
  })
})
