import data from './releases.json'
import type { Locale } from '~/i18n'

export type Category = 'added' | 'changed' | 'fixed' | 'maintenance'
export interface ReleaseItem {
  id: string
  category: Category
  text: Record<Locale, string>
  sources: string[]
}
export interface ReleaseNote {
  version: string
  status: 'released' | 'development' | 'prepared'
  publishedAt: string | null
  tags: string[]
  source: string | null
  items: ReleaseItem[]
}

// The release preflight validates these authored records before publication.
export const authoredNotes = data as ReleaseNote[]

export function publicReleaseNotes(publications: Record<string, string>) {
  return authoredNotes
    .filter((release) => release.items.length > 0 && release.status !== 'development')
    .flatMap((release): ReleaseNote[] => {
      if (release.status !== 'prepared' || release.version === 'unreleased') return [release]
      const tags = [`v${release.version}`, `macos-v${release.version}`].filter(
        (tag) => publications[tag],
      )
      const publishedAt = tags.map((tag) => publications[tag]!).sort()[0]
      return publishedAt ? [{ ...release, status: 'released', publishedAt, tags }] : []
    })
    .sort((left, right) => {
      if (left.version === 'unreleased') return -1
      if (right.version === 'unreleased') return 1
      return right.version.localeCompare(left.version, 'en', { numeric: true })
    })
}

export const categories: Category[] = ['added', 'changed', 'fixed', 'maintenance']
export const releaseNotesPath = (locale: Locale, version?: string) =>
  `${locale === 'en' ? '' : `/${locale}`}/docs/release-notes/${version ? `${version}/` : ''}`
export const isReleaseVersion = (version: unknown, publications: Record<string, string>) =>
  version === undefined ||
  version === '' ||
  publicReleaseNotes(publications).some((release) => release.version === version)
