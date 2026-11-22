<script setup lang="ts">
import { docsGroups, type DocsNavItem } from '~/docs'
import { docsPath, type Messages } from '~/i18n'

const props = withDefaults(
  defineProps<{
    messages: Messages
    slug?: DocsNavItem
  }>(),
  { slug: undefined },
)

const nav = computed(() =>
  docsGroups.map((group) => ({
    label: props.messages.docs.groups[group.key],
    items: group.items.map((item) => ({
      item,
      path: item === 'overview' ? docsPath() : docsPath(item),
      label:
        item === 'overview'
          ? props.messages.docs.overview.label
          : item === 'release-notes'
            ? props.messages.releaseNotes.label
            : props.messages.docs.pages[item].label,
      current: props.slug === item,
    })),
  })),
)
</script>

<template>
  <div>
    <SiteHeader :messages="messages" section="docs" />
    <div class="shell docs-shell">
      <details class="docs-mobile">
        <summary>{{ messages.docs.label }}</summary>
        <nav class="docs-nav" :aria-label="messages.docs.label">
          <div v-for="group in nav" :key="group.label" class="docs-group">
            <p class="docs-group-label">{{ group.label }}</p>
            <NuxtLink
              v-for="item in group.items"
              :key="item.item"
              :to="item.path"
              :aria-current="item.current ? 'page' : undefined"
              >{{ item.label }}</NuxtLink
            >
          </div>
        </nav>
      </details>
      <aside class="docs-aside">
        <nav class="docs-nav" :aria-label="messages.docs.label">
          <div v-for="group in nav" :key="group.label" class="docs-group">
            <p class="docs-group-label">{{ group.label }}</p>
            <NuxtLink
              v-for="item in group.items"
              :key="item.item"
              :to="item.path"
              :aria-current="item.current ? 'page' : undefined"
              >{{ item.label }}</NuxtLink
            >
          </div>
        </nav>
      </aside>
      <main id="main" class="docs-main">
        <slot />
      </main>
    </div>
    <SiteFooter :messages="messages" />
  </div>
</template>

<style scoped>
.docs-shell {
  display: grid;
  grid-template-columns: 210px minmax(0, 1fr);
  gap: 24px 56px;
  align-items: start;
  padding-block: 48px 110px;
}
.docs-aside {
  position: sticky;
  top: 24px;
}
.docs-group-label {
  font-size: 11px;
  font-weight: 550;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--quiet);
  margin: 28px 0 6px;
}
.docs-group:first-child .docs-group-label {
  margin-top: 0;
}
.docs-nav a {
  display: block;
  padding: 5px 0;
  font-size: 13px;
  color: var(--muted);
  line-height: 1.45;
}
.docs-nav a:hover {
  color: var(--text);
}
.docs-nav a[aria-current] {
  color: var(--text);
  font-weight: 550;
}
.docs-main {
  max-width: 720px;
}
.docs-mobile {
  display: none;
}
@media (max-width: 900px) {
  .docs-shell {
    grid-template-columns: minmax(0, 1fr);
    padding-block: 32px 64px;
  }
  .docs-aside {
    display: none;
  }
  .docs-mobile {
    display: block;
    margin-bottom: 32px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--surface);
  }
  .docs-mobile summary {
    padding: 12px 16px;
    font-size: 13px;
    font-weight: 550;
    cursor: pointer;
    list-style: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .docs-mobile summary::after {
    content: '+';
    color: var(--muted);
  }
  .docs-mobile[open] summary::after {
    content: '–';
  }
  .docs-mobile .docs-nav {
    padding: 4px 16px 14px;
  }
  .docs-mobile .docs-group-label {
    margin-top: 14px;
  }
}
</style>
