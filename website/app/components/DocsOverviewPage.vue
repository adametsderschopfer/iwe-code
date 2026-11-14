<script setup lang="ts">
import { docsGroups } from '~/docs'
import { catalogs, docsPath, languages, type Locale } from '~/i18n'

const route = useRoute()
const locale = computed<Locale>(() =>
  route.params.lang === 'ru' ? 'ru' : route.params.lang === 'es' ? 'es' : 'en',
)
const messages = computed(() => catalogs[locale.value])
const base = useRuntimeConfig().app.baseURL
const siteURL = 'https://adametsderschopfer.github.io/iwe-code/'
const canonical = computed(() => new URL(docsPath(locale.value).slice(1), siteURL).href)
useHead(() => ({
  htmlAttrs: { lang: locale.value },
  title: messages.value.docs.metaTitle,
  link: [
    { rel: 'icon', href: `${base}icon.webp`, type: 'image/webp' },
    { rel: 'canonical', href: canonical.value },
    ...languages.map((language) => ({
      rel: 'alternate' as const,
      hreflang: language.code,
      href: new URL(docsPath(language.code).slice(1), siteURL).href,
    })),
    { rel: 'alternate', hreflang: 'x-default', href: new URL('docs/', siteURL).href },
  ],
}))
useSeoMeta({
  description: () => messages.value.docs.overview.description,
  ogTitle: () => messages.value.docs.metaTitle,
  ogDescription: () => messages.value.docs.overview.description,
  ogType: 'website',
  ogUrl: () => canonical.value,
})

const groups = computed(() =>
  docsGroups.map((group) => ({
    label: messages.value.docs.groups[group.key],
    items: group.items.map((item) => ({
      item,
      path: item === 'overview' ? docsPath(locale.value) : docsPath(locale.value, item),
      label:
        item === 'overview'
          ? messages.value.docs.overview.label
          : item === 'release-notes'
            ? messages.value.releaseNotes.label
            : messages.value.docs.pages[item].label,
      description:
        item === 'overview'
          ? messages.value.docs.overview.description
          : item === 'release-notes'
            ? messages.value.releaseNotes.description
            : messages.value.docs.pages[item].description,
    })),
  })),
)
</script>

<template>
  <DocsLayout :messages="messages" :locale="locale">
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
