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
    .filter((release) => release.items.length > 0)
    .flatMap((release): ReleaseNote[] => {
      if (release.status !== 'prepared' || release.version === 'unreleased') return [release]
      const tag = `v${release.version}`
      const publishedAt = publications[tag]
      return publishedAt ? [{ ...release, status: 'released', publishedAt, tags: [tag] }] : []
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
