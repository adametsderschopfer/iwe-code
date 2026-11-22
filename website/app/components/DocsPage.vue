<script setup lang="ts">
import type { DocsSlug } from '~/docs'
import { messages, docsPath } from '~/i18n'

const props = defineProps<{ slug: DocsSlug }>()

const page = computed(() => messages.docs.pages[props.slug])
const base = useRuntimeConfig().app.baseURL
const siteURL = 'https://iwe-code.ru/'
const canonical = computed(() => new URL(docsPath(props.slug).slice(1), siteURL).href)
useHead(() => ({
  htmlAttrs: { lang: 'en' },
  title: page.value.metaTitle,
  link: [
    { rel: 'icon', href: `${base}icon.webp`, type: 'image/webp' },
    { rel: 'canonical', href: canonical.value },
  ],
}))
useSeoMeta({
  description: () => page.value.description,
  ogTitle: () => page.value.metaTitle,
  ogDescription: () => page.value.description,
  ogType: 'website',
  ogUrl: () => canonical.value,
})
</script>

<template>
  <DocsLayout :messages="messages" :slug="slug">
    <article class="docs-article">
      <h1>{{ page.title }}</h1>
      <p class="section-description">{{ page.description }}</p>
      <p class="docs-intro">{{ page.intro }}</p>
      <section v-for="(section, index) in page.sections" :key="index" class="docs-section">
        <h2>{{ section.heading }}</h2>
        <p v-for="(paragraph, pIndex) in section.paragraphs" :key="pIndex">{{ paragraph }}</p>
        <ul v-if="section.bullets?.length" class="docs-list">
          <li v-for="(bullet, bIndex) in section.bullets" :key="bIndex">{{ bullet }}</li>
        </ul>
        <ol v-if="section.steps?.length" class="docs-list">
          <li v-for="(step, sIndex) in section.steps" :key="sIndex">{{ step }}</li>
        </ol>
        <dl
          v-if="section.table"
          class="docs-table"
          :class="{ 'docs-table--stacked': section.table.kind === 'command' }"
        >
          <template v-for="(row, rIndex) in section.table.rows" :key="rIndex">
            <dt>
              <code v-if="section.table.kind !== 'text'">{{ row.term }}</code>
              <template v-else>{{ row.term }}</template>
            </dt>
            <dd>{{ row.text }}</dd>
          </template>
        </dl>
        <template v-if="section.codes?.length">
          <template v-for="(code, cIndex) in section.codes" :key="cIndex">
            <h3 v-if="code.label">{{ code.label }}</h3>
            <pre class="docs-code">{{ code.text }}</pre>
          </template>
        </template>
        <div v-if="section.links?.length" class="docs-links">
          <template v-for="(link, lIndex) in section.links" :key="lIndex">
            <NuxtLink v-if="link.page" class="text-link" :to="docsPath(link.page)"
              >{{ link.text }} <SiteIcon name="arrow"
            /></NuxtLink>
            <a v-else class="text-link" :href="link.href"
              >{{ link.text }} <SiteIcon name="arrow"
            /></a>
          </template>
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
.docs-section h3 {
  font-size: 14px;
  font-weight: 550;
  color: var(--muted);
  margin: 24px 0 0;
}
.docs-section p {
  margin: 12px 0 0;
  max-width: 700px;
  color: var(--muted);
  line-height: 1.7;
}
.docs-list {
  margin: 16px 0 0;
  padding-left: 20px;
  color: var(--muted);
  line-height: 1.8;
}
.docs-table {
  display: grid;
  grid-template-columns: minmax(140px, auto) 1fr;
  gap: 10px 24px;
  margin: 20px 0 0;
}
.docs-table dt {
  font-size: 13px;
}
.docs-table dt code {
  font-size: 12px;
  color: var(--text);
}
.docs-table dd {
  margin: 0;
  font-size: 13px;
  color: var(--muted);
  line-height: 1.6;
}
.docs-code {
  margin: 20px 0 0;
  padding: 18px 20px;
  overflow-x: auto;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--raised);
  font-size: 12px;
  line-height: 1.7;
  white-space: pre;
}
.docs-links {
  display: flex;
  flex-direction: column;
  gap: 12px;
  align-items: flex-start;
  margin-top: 20px;
  font-size: 13px;
}
.docs-links a {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.docs-links svg {
  width: 13px;
  height: 13px;
}
@media (max-width: 600px) {
  .docs-table {
    grid-template-columns: 1fr;
    gap: 4px;
  }
  .docs-table dd {
    margin-bottom: 10px;
  }
}
.docs-table--stacked {
  grid-template-columns: minmax(0, 1fr);
  gap: 0;
}
.docs-table--stacked dt {
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.docs-table--stacked dt code {
  font-size: 13px;
}
.docs-table--stacked dd {
  margin: 4px 0 0;
  font-size: 14px;
}
.docs-table--stacked dd:not(:last-child) {
  border-bottom: 1px solid var(--line);
  padding-bottom: 12px;
  margin-bottom: 12px;
}
</style>
