import process from 'node:process'
import { docsSlugs } from './app/docs'
import { publicReleaseNotes } from './app/release-notes'

const releasesURL = 'https://api.github.com/repos/adametsderschopfer/iwe-code/releases'
async function fetchReleases(path: string) {
  const response = await fetch(`${releasesURL}${path}`, {
    headers: {
      Accept: 'application/vnd.github+json',
      ...(process.env.GH_TOKEN ? { Authorization: `Bearer ${process.env.GH_TOKEN}` } : {}),
    },
    signal: AbortSignal.timeout(30_000),
  })
  if (!response.ok) {
    throw new Error(`Cannot load GitHub releases (${path}): HTTP ${response.status}`)
  }
  return response
}

interface GitHubRelease {
  tag_name: string
  draft: boolean
  prerelease: boolean
  published_at: string | null
  html_url: string
  assets: { name: string; browser_download_url: string; size: number }[]
}

const stableTag = /^(?:(macos|windows|linux)-)?v([0-9]+\.[0-9]+\.[0-9]+)$/
const publications: Record<string, string> = {}
const releases: { record: GitHubRelease; version: string; family?: string }[] = []
for (let page = 1; ; page++) {
  const response = await fetchReleases(`?per_page=100&page=${page}`)
  const records = (await response.json()) as GitHubRelease[]
  for (const record of records) {
    const match = stableTag.exec(record.tag_name)
    if (match && !record.draft && !record.prerelease && record.published_at) {
      publications[record.tag_name] = record.published_at
      releases.push({ record, version: match[2]!, family: match[1] })
    }
  }
  if (!response.headers.get('link')?.includes('rel="next"')) break
}
releases.sort((left, right) => right.version.localeCompare(left.version, 'en', { numeric: true }))
const latest = releases[0]
if (!latest) throw new Error('No published stable GitHub release found')

const required = {
  macos: ['macos-arm64.dmg', 'macos-x64.dmg'],
  windows: ['windows-x64-setup.exe', 'windows-x86-setup.exe'],
  linux: ['linux-x64.tar.gz', 'linux-x86.tar.gz'],
}
const downloads: Record<string, string> = {}
const versions: Record<string, string> = {}
for (const [family, suffixes] of Object.entries(required)) {
  const optional =
    family === 'linux' ? ['linux-x64.deb', 'linux-x86.deb', 'linux-x64.AppImage'] : []
  for (const { record, version, family: channel } of releases) {
    if (channel && channel !== family) continue
    const files = Object.fromEntries(
      [...suffixes, ...optional].flatMap((suffix) => {
        const asset = record.assets.find(
          (item) => item.size > 0 && item.name === `iwe-code-${version}-${suffix}`,
        )
        return asset ? [[suffix, asset.browser_download_url]] : []
      }),
    )
    if (!suffixes.every((suffix) => files[suffix])) continue
    Object.assign(downloads, files)
    versions[family] = version
    break
  }
}
const releaseNotes = publicReleaseNotes(publications)
const version = latest.version

export default defineNuxtConfig({
  runtimeConfig: {
    public: {
      releasePublications: publications,
      downloadRelease: {
        version,
        url: latest.record.html_url,
        hasNotes: releaseNotes.some((record) => record.version === version),
        downloads,
        versions,
      },
    },
  },
  compatibilityDate: '2026-11-09',
  devtools: { enabled: false },
  modules: ['@nuxt/eslint'],
  css: ['~/assets/main.css'],
  app: {
    head: {
      meta: [{ name: 'theme-color', content: '#121311' }],
    },
  },
  nitro: {
    prerender: {
      routes: [
        '/',
        '/ru/',
        '/es/',
        ...['', '/ru', '/es'].flatMap((locale) => [
          `${locale}/docs/`,
          ...docsSlugs.map((slug) => `${locale}/docs/${slug}/`),
          `${locale}/docs/release-notes/`,
          ...releaseNotes.map((release) => `${locale}/docs/release-notes/${release.version}/`),
          `${locale}/license/`,
          `${locale}/roadmap/`,
        ]),
      ],
      failOnError: true,
    },
  },
  typescript: { strict: true },
})
