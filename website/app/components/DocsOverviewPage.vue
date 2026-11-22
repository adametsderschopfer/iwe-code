<script setup lang="ts">
import { docsGroups } from '~/docs'
import { messages, docsPath } from '~/i18n'

const base = useRuntimeConfig().app.baseURL
const siteURL = 'https://adametsderschopfer.github.io/iwe-code/'
const canonical = computed(() => new URL(docsPath().slice(1), siteURL).href)
useHead(() => ({
  htmlAttrs: { lang: 'en' },
  title: messages.docs.metaTitle,
  link: [
    { rel: 'icon', href: `${base}icon.webp`, type: 'image/webp' },
    { rel: 'canonical', href: canonical.value },
  ],
}))
useSeoMeta({
  description: () => messages.docs.overview.description,
  ogTitle: () => messages.docs.metaTitle,
  ogDescription: () => messages.docs.overview.description,
  ogType: 'website',
  ogUrl: () => canonical.value,
})

const groups = computed(() =>
  docsGroups.map((group) => ({
    label: messages.docs.groups[group.key],
    items: group.items.map((item) => ({
      item,
      path: item === 'overview' ? docsPath() : docsPath(item),
      label:
        item === 'overview'
          ? messages.docs.overview.label
          : item === 'release-notes'
            ? messages.releaseNotes.label
            : messages.docs.pages[item].label,
      description:
        item === 'overview'
          ? messages.docs.overview.description
          : item === 'release-notes'
            ? messages.releaseNotes.description
            : messages.docs.pages[item].description,
    })),
  })),
)
</script>

<template>
  <DocsLayout :messages="messages">
    <article class="docs-article">
      <h1>{{ messages.docs.overview.title }}</h1>
      <p class="section-description">{{ messages.docs.overview.description }}</p>
      <p class="docs-intro">{{ messages.docs.overview.intro }}</p>
      <section v-for="group in groups" :key="group.label" class="docs-section">
        <h2>{{ group.label }}</h2>
        <div class="overview-grid">
          <NuxtLink
            v-for="item in group.items"
            :key="item.item"
            class="overview-item"
            :to="item.path"
          >
            <span class="overview-label">{{ item.label }}</span>
            <span class="overview-text">{{ item.description }}</span>
          </NuxtLink>
        </div>
      </section>
    </article>
  </DocsLayout>
</template>

<style scoped>
.docs-article h1 {
  font-size: clamp(36px, 5vw, 52px);
  font-weight: 500;
  letter-spacing: -0.04em;
  line-height: 1.1;
  margin: 0;
  white-space: pre-line;
}
.docs-intro {
  margin-top: 24px;
  max-width: 640px;
  font-size: 15px;
  line-height: 1.7;
}
.docs-section {
  border-top: 1px solid var(--line);
  margin-top: 44px;
  padding-top: 32px;
}
.docs-section h2 {
  font-size: 22px;
  font-weight: 550;
  letter-spacing: -0.02em;
  margin: 0;
}
.overview-grid {
  display: grid;
  gap: 10px;
  margin-top: 20px;
}
.overview-item {
  display: grid;
  gap: 4px;
  padding: 14px 18px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--surface);
}
.overview-item:hover {
  border-color: var(--line-strong);
}
.overview-label {
  font-size: 14px;
  font-weight: 550;
}
.overview-text {
  font-size: 13px;
  color: var(--muted);
  line-height: 1.6;
}
</style>
