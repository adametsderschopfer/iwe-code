<script setup lang="ts">
import { catalogs, docsPath, type Locale } from '~/i18n'
import { isReleaseVersion } from '~/release-notes'

const props = defineProps<{ error: { url?: string; statusCode?: number } }>()

const currentPath = ref<string>()
onMounted(() => {
  currentPath.value = window.location.pathname
})

// Old section pages moved under /docs/. Send their known addresses to the new pages.
const legacyTarget = computed(() => {
  const url = currentPath.value ?? props.error?.url ?? ''
  const match = url.match(/^\/(ru|es)?\/?(cli|mcp|modules|external|releases)(?:\/([^/]+))?\/?$/)
  if (!match?.[2]) return null
  const locale: Locale = match[1] === 'ru' ? 'ru' : match[1] === 'es' ? 'es' : 'en'
  const slugs: Record<string, string> = {
    cli: 'cli',
    mcp: 'mcp',
    modules: 'module-development',
    external: 'deep-links',
    releases: 'release-notes',
  }
  const rest = match[3]
  if (rest && (match[2] !== 'releases' || !isReleaseVersion(rest))) return null
  return docsPath(locale, slugs[match[2]], rest ? `${rest}/` : '')
})

onMounted(() => {
  if (legacyTarget.value) clearError({ redirect: legacyTarget.value })
})

const route = useRoute()
const messages = computed(() =>
  route.path.startsWith('/ru')
    ? catalogs.ru
    : route.path.startsWith('/es')
      ? catalogs.es
      : catalogs.en,
)
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
