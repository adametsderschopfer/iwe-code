<script setup lang="ts">
import licenseText from '../../../LICENSE?raw'
import { messages, licensePath, repository } from '~/i18n'

const base = useRuntimeConfig().app.baseURL
const siteURL = 'https://adametsderschopfer.github.io/iwe-code/'
const canonical = computed(() => new URL(licensePath.slice(1), siteURL).href)
useHead(() => ({
  htmlAttrs: { lang: 'en' },
  title: messages.license.metaTitle,
  link: [
    { rel: 'icon', href: `${base}icon.webp`, type: 'image/webp' },
    { rel: 'canonical', href: canonical.value },
  ],
}))
useSeoMeta({
  description: () => messages.license.description,
  ogTitle: () => messages.license.metaTitle,
  ogDescription: () => messages.license.description,
  ogType: 'website',
  ogUrl: () => canonical.value,
})
</script>

<template>
  <div>
    <SiteHeader :messages="messages" section="license" />
    <main id="main" class="shell license-page">
      <h1>{{ messages.license.title }}</h1>
      <p class="section-description">{{ messages.license.description }}</p>
      <a class="text-link license-source" :href="`${repository}/blob/main/LICENSE`"
        >{{ messages.license.source }} <SiteIcon name="arrow"
      /></a>
      <pre class="license-text" lang="en" :aria-label="messages.license.fullText">{{
        licenseText
      }}</pre>
    </main>
    <SiteFooter :messages="messages" />
  </div>
</template>

<style scoped>
.license-page {
  max-width: 960px;
  padding-block: 72px 110px;
}
h1 {
  font-size: clamp(36px, 6vw, 60px);
  font-weight: 500;
  letter-spacing: -0.05em;
  margin: 0;
}
.license-source {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 24px;
  font-size: 13px;
}
.license-source svg {
  width: 13px;
  height: 13px;
}
.license-text {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  font-size: 12px;
  line-height: 1.75;
  color: var(--muted);
  border-top: 1px solid var(--line);
  margin: 40px 0 0;
  padding-top: 32px;
}
@media (max-width: 600px) {
  .license-page {
    padding-block: 44px 64px;
  }
}
</style>
