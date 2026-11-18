<script setup lang="ts">
import { roadmapPath, type Locale, type Messages } from '~/i18n'
import { publicReleaseNotes, releaseNotesPath } from '~/release-notes'

const {
  messages,
  locale,
  preview = false,
} = defineProps<{
  messages: Messages
  locale: Locale
  preview?: boolean
}>()

const publications = useRuntimeConfig().public.releasePublications
const plans = computed(() => {
  const tracks = Object.entries(messages.roadmap.tracks)
  return preview ? tracks.slice(0, 2) : tracks
})
const releases = computed(() => {
  const format = new Intl.DateTimeFormat(locale, { dateStyle: 'long', timeZone: 'UTC' })
  return publicReleaseNotes(publications)
    .filter((release) => release.status === 'released' && release.publishedAt)
    .sort((left, right) => right.publishedAt!.localeCompare(left.publishedAt!))
    .slice(0, preview ? 1 : undefined)
    .map((release) => {
      const highlights = release.items.filter((item) => item.category === 'added')
      return {
        version: release.version,
        date: release.publishedAt,
        dateLabel: format.format(new Date(release.publishedAt!)),
        label:
          release.tags.length && release.tags.every((tag) => tag.startsWith('macos-'))
            ? `macOS ${release.version}`
            : `v${release.version}`,
        highlights: (highlights.length ? highlights : release.items).slice(0, preview ? 1 : 3),
      }
    })
})
</script>

<template>
  <section
    id="roadmap"
    class="roadmap"
    :class="{ 'roadmap-preview section-space': preview }"
    aria-labelledby="roadmap-title"
  >
    <header class="roadmap-heading">
      <component :is="preview ? 'h2' : 'h1'" id="roadmap-title">
        {{ preview ? messages.roadmap.title : messages.roadmap.nav }}
      </component>
      <p>{{ messages.roadmap.description }}</p>
    </header>

    <div class="roadmap-board">
      <section class="roadmap-group" aria-labelledby="roadmap-planned">
        <div class="roadmap-group-label">
          <component :is="preview ? 'h3' : 'h2'" id="roadmap-planned">
            {{ messages.roadmap.planned }}
          </component>
        </div>
        <ul class="roadmap-list">
          <li v-for="[id, plan] in plans" :key="id" class="roadmap-row">
            <span class="roadmap-tag roadmap-tag-topic">{{ plan.title }}</span>
            <component :is="preview ? 'h4' : 'h3'">{{ plan.next }}</component>
            <p>{{ plan.detail }}</p>
          </li>
        </ul>
      </section>

      <section v-if="releases.length" class="roadmap-group" aria-labelledby="roadmap-released">
        <div class="roadmap-group-label">
          <component :is="preview ? 'h3' : 'h2'" id="roadmap-released">
            {{ preview ? messages.roadmap.recent : messages.roadmap.shipped }}
          </component>
        </div>
        <ol class="roadmap-list roadmap-timeline">
          <li
            v-for="release in releases"
            :key="release.version"
            class="roadmap-row roadmap-release"
          >
            <div class="roadmap-release-header">
              <time :datetime="release.date ?? undefined">{{ release.dateLabel }}</time>
              <component :is="preview ? 'h4' : 'h3'">
                <NuxtLink
                  class="roadmap-tag roadmap-tag-release"
                  :to="releaseNotesPath(locale, release.version)"
                >
                  {{ release.label }}
                </NuxtLink>
              </component>
            </div>
            <ul class="roadmap-highlights">
              <li v-for="item in release.highlights" :key="item.id">{{ item.text[locale] }}</li>
            </ul>
          </li>
        </ol>
      </section>
    </div>

    <NuxtLink v-if="preview" class="roadmap-link" :to="roadmapPath(locale)">
      {{ messages.roadmap.full }} <SiteIcon name="arrow" />
    </NuxtLink>
  </section>
</template>

<style scoped>
.roadmap-preview {
  border-top: 1px solid var(--line);
}
.roadmap-heading h1,
.roadmap-heading h2 {
  margin: 0;
  font-size: clamp(36px, 6vw, 60px);
  font-weight: 500;
  letter-spacing: -0.05em;
  line-height: 1.1;
}
.roadmap-preview .roadmap-heading h2 {
  font-size: clamp(34px, 4vw, 48px);
  letter-spacing: -0.045em;
}
.roadmap-heading p {
  max-width: 560px;
  margin: 18px 0 0;
  color: var(--muted);
  font-size: 15px;
  line-height: 1.65;
}
.roadmap-board {
  margin-top: 40px;
  padding: 0 30px;
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  background: var(--surface);
}
.roadmap-group {
  display: grid;
  grid-template-columns: minmax(0, 190px) minmax(0, 1fr);
  gap: 24px;
  padding: 28px 0;
}
.roadmap-group + .roadmap-group {
  border-top: 1px solid var(--line);
}
.roadmap-group-label h2,
.roadmap-group-label h3 {
  margin: 0;
  color: var(--text);
  font-size: 16px;
  font-weight: 550;
}
.roadmap-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.roadmap-row + .roadmap-row {
  margin-top: 23px;
  padding-top: 23px;
  border-top: 1px solid var(--line);
}
.roadmap-row :is(h3, h4) {
  margin: 8px 0 0;
  font-size: 17px;
  font-weight: 550;
  line-height: 1.3;
  letter-spacing: -0.025em;
}
.roadmap-row p,
.roadmap-highlights {
  max-width: 65ch;
  margin: 5px 0 0;
  color: var(--muted);
  font-size: 14px;
  line-height: 1.6;
}
.roadmap-tag {
  display: inline-flex;
  padding: 3px 10px;
  border: 1px solid var(--line-strong);
  border-radius: 5px;
  font-size: 11px;
  font-weight: 550;
  line-height: 1.6;
}
.roadmap-tag-topic {
  background: var(--raised);
  color: var(--text);
}
.roadmap-tag-release {
  color: var(--accent);
}
.roadmap-tag-release:hover {
  color: var(--text);
  text-decoration: underline;
}
.roadmap-release-header {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
}
.roadmap-release-header :is(h3, h4) {
  margin: 0;
  font-size: 14px;
  letter-spacing: 0;
}
.roadmap-release time {
  font-size: 13px;
  color: var(--muted);
}
.roadmap-highlights {
  padding-inline-start: 1.1em;
}
.roadmap-highlights li + li {
  margin-top: 4px;
}
.roadmap-link {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  margin-top: 20px;
  color: var(--accent);
  font-size: 14px;
  font-weight: 550;
}
.roadmap-link:hover {
  text-decoration: underline;
}
.roadmap-link svg {
  width: 15px;
  height: 15px;
}
@media (max-width: 680px) {
  .roadmap-board {
    padding: 0 20px;
  }
  .roadmap-group {
    grid-template-columns: 1fr;
    gap: 18px;
    padding: 22px 0;
  }
}
</style>
