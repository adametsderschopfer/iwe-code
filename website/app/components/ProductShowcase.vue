<script setup lang="ts">
import type { Messages } from '~/i18n'

const { messages } = defineProps<{ messages: Messages }>()
const selected = shallowRef('workspace')
const base = useRuntimeConfig().app.baseURL
const tabs = computed(() => [
  {
    id: 'workspace',
    title: messages.preview.workspace,
    alt: messages.preview.altWorkspace,
    width: 2880,
    height: 2000,
  },
  {
    id: 'search',
    title: messages.preview.search,
    alt: messages.preview.altSearch,
    width: 2880,
    height: 1840,
  },
  {
    id: 'history',
    title: messages.preview.history,
    alt: messages.preview.altHistory,
    width: 2880,
    height: 1840,
  },
  {
    id: 'comparison',
    title: messages.preview.comparison,
    alt: messages.preview.altComparison,
    width: 2880,
    height: 1840,
  },
  {
    id: 'settings',
    title: messages.preview.settings,
    alt: messages.preview.altSettings,
    width: 2880,
    height: 1840,
  },
])
</script>

<template>
  <figure class="showcase">
    <div class="showcase-toolbar">
      <div class="preview-options" role="group" :aria-label="messages.preview.label">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          type="button"
          :aria-pressed="selected === tab.id"
          aria-controls="product-preview"
          @click="selected = tab.id"
        >
          {{ tab.title }}
        </button>
      </div>
      <span class="preview-product">IWE Code</span>
    </div>
    <div class="preview-stage">
      <ScrollReveal id="product-preview" class="preview-image">
        <template v-for="tab in tabs" :key="tab.id">
          <img
            v-show="selected === tab.id"
            :src="`${base}images/${tab.id}-en.webp`"
            :alt="tab.alt"
            :width="tab.width"
            :height="tab.height"
            :loading="tab.id === 'workspace' ? 'eager' : 'lazy'"
            :fetchpriority="tab.id === 'workspace' ? 'high' : 'auto'"
          />
        </template>
      </ScrollReveal>
    </div>
    <figcaption>{{ messages.preview.caption }}</figcaption>
  </figure>
</template>

<style scoped>
.showcase {
  margin: clamp(94px, 10vw, 152px) 0 0;
  position: relative;
}
.showcase-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}
.preview-options {
  display: flex;
  gap: 4px;
  padding: 4px;
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 9px;
  min-width: 0;
  overflow-x: auto;
}
.preview-options button {
  padding: 8px 14px;
  border: 0;
  background: transparent;
  color: var(--muted);
  border-radius: 5px;
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
}
.preview-options button[aria-pressed='true'] {
  background: var(--raised);
  color: var(--text);
  box-shadow: 0 1px 4px #0003;
}
.preview-product {
  color: var(--muted);
  font-size: 12px;
}
.preview-image {
  aspect-ratio: 1.44;
  overflow: hidden;
  border: 1px solid #444540;
  border-radius: 9px;
  background: #212121;
  box-shadow: 0 25px 90px #0004;
}
.preview-image img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  object-position: center;
}
.showcase figcaption {
  text-align: center;
  margin-top: 20px;
  font-size: 12px;
  color: var(--muted);
}
@media (prefers-reduced-motion: no-preference) {
  .preview-image.reveal-rise {
    transform: none;
  }
  .preview-image.reveal-rise[data-reveal-state='pending'] {
    transform: translateY(6%) scale(0.92);
  }
  @starting-style {
    .preview-image.reveal-rise[data-reveal-state='visible'] {
      opacity: 0;
      transform: translateY(6%) scale(0.92);
    }
  }
}
@supports ((animation-timeline: view()) and (animation-range: entry)) {
  @media (prefers-reduced-motion: no-preference) {
    .preview-stage {
      transform-origin: center top;
      animation: preview-settle auto linear both;
      animation-timeline: view();
      animation-range: entry 0% entry 80%;
    }
    .preview-image.reveal-rise[data-reveal-state] {
      opacity: 1;
      transform: none;
      transition: none;
    }
    @media (max-width: 650px) {
      .preview-stage {
        animation-name: preview-fade;
      }
    }
  }
}
@keyframes preview-settle {
  from {
    opacity: 0.18;
    transform: perspective(1400px) translate3d(0, 6%, -240px) rotateX(26deg) scale(0.88);
  }
  to {
    opacity: 1;
    transform: perspective(1400px) translate3d(0, 0, 0) rotateX(0) scale(1);
  }
}
@media (max-width: 650px) {
  .showcase {
    margin-top: 68px;
  }
  .preview-options {
    width: 100%;
    overflow-x: auto;
  }
  .preview-options button {
    flex: 1;
    padding: 8px 10px;
  }
  .preview-product {
    display: none;
  }
}
@keyframes preview-fade {
  from {
    opacity: 0.28;
    transform: translateY(16px) scale(0.96);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
</style>
