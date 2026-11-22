<script setup lang="ts">
import { docsPath, repository, roadmapPath, type Messages } from '~/i18n'

withDefaults(
  defineProps<{
    messages: Messages
    section?: 'home' | 'docs' | 'license' | 'roadmap'
  }>(),
  { section: 'home' },
)
const base = useRuntimeConfig().app.baseURL
const homeHref = base
</script>

<template>
  <a class="skip-link" href="#main">{{ messages.nav.skip }}</a>
  <header class="site-header shell">
    <NuxtLink class="brand" to="/" :aria-label="messages.nav.home">
      <img :src="`${base}icon.webp`" width="36" height="36" alt="" />
      <span>IWE <span class="brand-code">Code</span></span>
    </NuxtLink>
    <nav class="main-nav" :aria-label="messages.nav.features">
      <a :href="section === 'home' ? '#features' : `${homeHref}#features`">{{
        messages.nav.features
      }}</a>
      <a :href="section === 'home' ? '#workflow' : `${homeHref}#workflow`">{{
        messages.nav.workflow
      }}</a>
      <NuxtLink :to="roadmapPath" :aria-current="section === 'roadmap' ? 'page' : undefined">
        {{ messages.roadmap.nav }}
      </NuxtLink>
      <NuxtLink :to="docsPath()" :aria-current="section === 'docs' ? 'page' : undefined">{{
        messages.docs.label
      }}</NuxtLink>
      <a :href="repository">GitHub <SiteIcon name="arrow" /></a>
    </nav>
    <div class="header-actions">
      <a
        class="button button-small header-download"
        :href="section === 'home' ? '#download' : `${homeHref}#download`"
        >{{ messages.nav.download }}</a
      >
    </div>
  </header>
</template>

<style scoped>
.site-header {
  display: flex;
  align-items: center;
  min-height: 96px;
  gap: 32px;
  border-bottom: 1px solid var(--line);
}
.brand {
  display: inline-flex;
  gap: 10px;
  align-items: center;
  white-space: nowrap;
  font-size: 21px;
  font-weight: 650;
  letter-spacing: -0.8px;
}
.brand-code {
  color: var(--muted);
  font-weight: 450;
}
.main-nav {
  display: flex;
  gap: 20px;
  font-size: 13px;
  color: var(--muted);
}
.main-nav a {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.main-nav svg {
  width: 13px;
  height: 13px;
}
.header-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
}
@media (max-width: 900px) {
  .main-nav {
    display: none;
  }
  .site-header {
    min-height: 78px;
  }
}
@media (max-width: 480px) {
  .site-header {
    gap: 16px;
  }
}
</style>
