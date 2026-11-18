<script setup lang="ts">
import { catalogs, languages, roadmapPath, type Locale } from '~/i18n'

const route = useRoute()
const locale = computed<Locale>(() =>
  route.params.lang === 'ru' ? 'ru' : route.params.lang === 'es' ? 'es' : 'en',
)
const messages = computed(() => catalogs[locale.value])
const base = useRuntimeConfig().app.baseURL
const siteURL = 'https://adametsderschopfer.github.io/iwe-code/'
const absolutePath = (language: Locale) => new URL(roadmapPath(language).slice(1), siteURL).href

useHead(() => ({
  htmlAttrs: { lang: locale.value },
  link: [
    { rel: 'icon', href: `${base}icon.webp`, type: 'image/webp' },
    { rel: 'canonical', href: absolutePath(locale.value) },
    ...languages.map((language) => ({
      rel: 'alternate' as const,
      type: 'text/html',
      hreflang: language.code,
      href: absolutePath(language.code),
    })),
    { rel: 'alternate', hreflang: 'x-default', href: absolutePath('en') },
  ],
}))
useSeoMeta({
  title: () => `${messages.value.roadmap.nav} · IWE Code`,
  description: () => messages.value.roadmap.description,
})
</script>

<template>
  <div>
    <SiteHeader :messages="messages" :locale="locale" section="roadmap" />
    <main id="main" class="shell roadmap-page section-space">
      <RoadmapSection :messages="messages" :locale="locale" />
    </main>
    <SiteFooter :messages="messages" :locale="locale" />
  </div>
</template>

<style scoped>
.roadmap-page {
  padding-top: 36px;
}
@media (max-width: 600px) {
  .roadmap-page {
    padding-top: 26px;
  }
}
</style>
