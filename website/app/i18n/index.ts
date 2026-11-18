import en from './en.json'
import ru from './ru.json'
import es from './es.json'
import type { DocsContent } from '~/docs'

export type Locale = 'en' | 'ru' | 'es'
export type Messages = Omit<typeof en, 'docs'> & { docs: DocsContent }

export const catalogs: Record<Locale, Messages> = { en, ru, es }
export const languages = [
  { code: 'en', label: 'EN', name: 'English', path: '/' },
  { code: 'ru', label: 'RU', name: 'Русский', path: '/ru/' },
  { code: 'es', label: 'ES', name: 'Español', path: '/es/' },
] as const

export const repository = 'https://github.com/adametsderschopfer/iwe-code'
export const releases = `${repository}/releases/latest`

export const roadmapPath = (locale: Locale) => `${locale === 'en' ? '' : `/${locale}`}/roadmap/`

export const licensePath = (locale: Locale) => `${locale === 'en' ? '' : `/${locale}`}/license/`

export const docsPath = (locale: Locale, slug?: string, suffix = '') =>
  `${locale === 'en' ? '' : `/${locale}`}/docs/${slug ? `${slug}/` : ''}${suffix}`

export const localizedPath = (path: string, locale: Locale) => {
  const stripped = path.replace(/^\/(?:ru|es)(?=\/|$)/, '')
  const normalized =
    stripped === '' || stripped === '/' ? '/' : stripped.endsWith('/') ? stripped : `${stripped}/`
  return locale === 'en' ? normalized : `/${locale}${normalized === '/' ? '' : normalized}`
}
