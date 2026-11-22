<script setup lang="ts">
import { messages, roadmapPath } from '~/i18n'

const base = useRuntimeConfig().app.baseURL
const siteURL = 'https://iwe-code.ru/'
const canonical = new URL(roadmapPath.slice(1), siteURL).href

useHead(() => ({
  htmlAttrs: { lang: 'en' },
  link: [
    { rel: 'icon', href: `${base}icon.webp`, type: 'image/webp' },
    { rel: 'canonical', href: canonical },
  ],
}))
useSeoMeta({
  title: () => `${messages.roadmap.nav} · IWE Code`,
  description: () => messages.roadmap.description,
})
</script>

<template>
  <div>
    <SiteHeader :messages="messages" section="roadmap" />
    <main id="main" class="shell roadmap-page section-space">
      <RoadmapSection :messages="messages" />
    </main>
    <SiteFooter :messages="messages" />
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
