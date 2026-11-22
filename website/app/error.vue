<script setup lang="ts">
import { docsSlugs } from '~/docs'
import { docsPath, messages } from '~/i18n'
import { isReleaseVersion } from '~/release-notes'

const props = defineProps<{ error: { url?: string; statusCode?: number } }>()

const {
  app,
  public: { releasePublications },
} = useRuntimeConfig()
useHead({ htmlAttrs: { lang: 'en' } })
const currentPath = ref<string>()
onMounted(() => {
  currentPath.value = window.location.pathname
})

// Old section pages moved under /docs/. Send their known addresses to the new pages.
const legacyTarget = computed(() => {
  const path = (currentPath.value ?? props.error?.url ?? '').split(/[?#]/, 1)[0] ?? ''
  const base = app.baseURL.replace(/\/$/, '')
  const url = base && path.startsWith(`${base}/`) ? path.slice(base.length) : path
  const formerLocale = /^\/(?:ru|es)(?=\/|$)/
  const englishPath = url.replace(formerLocale, '')
  const normalized = `${englishPath.replace(/\/$/, '')}/`
  if (formerLocale.test(url)) {
    const current = [
      '/',
      '/license/',
      '/roadmap/',
      '/docs/',
      ...docsSlugs.map((slug) => docsPath(slug)),
    ]
    if (current.includes(normalized)) return normalized
    const notes = normalized.match(/^\/docs\/release-notes\/(?:([^/]+)\/)?$/)
    if (notes && isReleaseVersion(notes[1], releasePublications)) return normalized
  }
  const match = englishPath.match(/^\/(cli|mcp|modules|external|releases)(?:\/([^/]+))?\/?$/)
  if (!match?.[1]) return null
  const slugs: Record<string, string> = {
    cli: 'cli',
    mcp: 'mcp',
    modules: 'module-development',
    external: 'deep-links',
    releases: 'release-notes',
  }
  const rest = match[2]
  if (rest && (match[1] !== 'releases' || !isReleaseVersion(rest, releasePublications))) return null
  return docsPath(slugs[match[1]]!, rest ? `${rest}/` : '')
})

onMounted(() => {
  if (legacyTarget.value) clearError({ redirect: legacyTarget.value })
})
</script>

<template>
  <main class="error-page shell">
    <template v-if="legacyTarget">
      <h1>{{ messages.docs.label }}</h1>
      <p>{{ messages.error.redirect }}</p>
      <NuxtLink class="button" :to="legacyTarget">{{ messages.docs.label }}</NuxtLink>
    </template>
    <template v-else>
      <h1>{{ messages.error.title }}</h1>
      <p>{{ messages.error.description }}</p>
      <NuxtLink class="button" to="/">{{ messages.error.back }}</NuxtLink>
    </template>
  </main>
</template>

<style scoped>
.error-page {
  padding-top: 20vh;
}
.error-page h1 {
  font-size: clamp(32px, 6vw, 64px);
  font-weight: 500;
  letter-spacing: -0.04em;
}
.error-page p {
  margin: 24px 0 32px;
  color: var(--muted);
}
</style>
