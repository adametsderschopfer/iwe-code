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

const release = (await (await fetchReleases('/latest')).json()) as GitHubRelease
if (release.draft || release.prerelease || !/^v[0-9]+\.[0-9]+\.[0-9]+$/.test(release.tag_name)) {
  throw new Error('The latest GitHub release must be a published stable version')
}
const publications: Record<string, string> = {}
for (let page = 1; ; page++) {
  const response = await fetchReleases(`?per_page=100&page=${page}`)
  const records = (await response.json()) as GitHubRelease[]
  for (const record of records) {
    if (
      !record.draft &&
      !record.prerelease &&
      record.published_at &&
      /^v[0-9]+\.[0-9]+\.[0-9]+$/.test(record.tag_name)
    ) {
      publications[record.tag_name] = record.published_at
    }
  }
  if (!response.headers.get('link')?.includes('rel="next"')) break
}
const releaseNotes = publicReleaseNotes(publications)
const version = release.tag_name.slice(1)
const downloads: Record<string, string> = {}
// The six established packages must exist in every published release.
for (const suffix of [
  'macos-arm64.dmg',
  'macos-x64.dmg',
  'windows-x64-setup.exe',
  'windows-x86-setup.exe',
  'linux-x64.tar.gz',
  'linux-x86.tar.gz',
]) {
  const asset = release.assets.find((item) => item.name === `iwe-code-${version}-${suffix}`)
  if (!asset || asset.size <= 0) {
    throw new Error(`The latest release is missing the download package: ${suffix}`)
  }
  downloads[suffix] = asset.browser_download_url
}
// Packages added later stay optional until a release ships them.
for (const suffix of ['linux-x64.deb', 'linux-x86.deb', 'linux-x64.AppImage']) {
  const asset = release.assets.find((item) => item.name === `iwe-code-${version}-${suffix}`)
  if (asset && asset.size > 0) {
    downloads[suffix] = asset.browser_download_url
  }
}

export default defineNuxtConfig({
  runtimeConfig: {
    public: {
      releasePublications: publications,
      downloadRelease: {
        version,
        url: release.html_url,
        hasNotes: releaseNotes.some((record) => record.version === version),
        downloads,
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
        ]),
      ],
      failOnError: true,
    },
  },
  typescript: { strict: true },
})
