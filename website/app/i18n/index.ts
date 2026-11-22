import en from './en.json'
import type { DocsContent } from '~/docs'

export type Messages = Omit<typeof en, 'docs'> & { docs: DocsContent }
export const messages: Messages = en

export const repository = 'https://github.com/adametsderschopfer/iwe-code'
export const releases = `${repository}/releases/latest`

export const roadmapPath = '/roadmap/'
export const licensePath = '/license/'
export const docsPath = (slug?: string, suffix = '') => `/docs/${slug ? `${slug}/` : ''}${suffix}`
