export const docsSlugs = [
  'getting-started',
  'workspaces',
  'editing',
  'git',
  'pipelines',
  'terminal',
  'deployment',
  'modules',
  'module-development',
  'cli',
  'mcp',
  'deep-links',
] as const

export type DocsSlug = (typeof docsSlugs)[number]
export type DocsNavItem = DocsSlug | 'overview' | 'release-notes'

export interface DocsTable {
  kind: string
  rows: { term: string; text: string }[]
}

export interface DocsLink {
  text: string
  href?: string
  page?: string
}

export interface DocsCode {
  label?: string
  text: string
}

export interface DocsSection {
  heading: string
  paragraphs?: string[]
  bullets?: string[]
  steps?: string[]
  table?: DocsTable
  codes?: DocsCode[]
  links?: DocsLink[]
}

export interface DocsPageContent {
  label: string
  title: string
  metaTitle: string
  description: string
  intro: string
  sections: DocsSection[]
}

export interface DocsOverviewContent {
  label: string
  title: string
  description: string
  intro: string
}

export interface DocsGroups {
  start: string
  workspace: string
  tools: string
  modules: string
  automation: string
  releases: string
}

export interface DocsContent {
  label: string
  metaTitle: string
  overview: DocsOverviewContent
  groups: DocsGroups
  pages: Record<DocsSlug, DocsPageContent>
}

export const docsGroups: { key: keyof DocsGroups; items: DocsNavItem[] }[] = [
  { key: 'start', items: ['overview', 'getting-started'] },
  { key: 'workspace', items: ['workspaces', 'editing'] },
  { key: 'tools', items: ['git', 'pipelines', 'terminal', 'deployment'] },
  { key: 'modules', items: ['modules', 'module-development'] },
  { key: 'automation', items: ['cli', 'mcp', 'deep-links'] },
  { key: 'releases', items: ['release-notes'] },
]
