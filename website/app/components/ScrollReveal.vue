<script setup lang="ts">
const {
  as = 'div',
  variant = 'rise',
  delay = 0,
} = defineProps<{
  as?: keyof HTMLElementTagNameMap
  variant?: 'rise' | 'wipe' | 'scene'
  delay?: number
}>()

const element = useTemplateRef<HTMLElement>('element')
const pending = shallowRef(false)
const immediate = shallowRef(false)
let observer: IntersectionObserver | undefined
let preference: MediaQueryList | undefined

function reveal() {
  pending.value = false
  observer?.disconnect()
}

function updatePreference() {
  if (preference?.matches) revealImmediately()
}

function revealImmediately() {
  immediate.value = true
  reveal()
}

onMounted(() => {
  preference = window.matchMedia('(prefers-reduced-motion: reduce)')
  if (preference.matches || !('IntersectionObserver' in window) || !element.value) return

  preference.addEventListener('change', updatePreference)
  if (element.value.getBoundingClientRect().top < window.innerHeight - 64) return

  observer = new IntersectionObserver(
    (entries) => {
      if (entries.some((entry) => entry.isIntersecting)) reveal()
    },
    { rootMargin: '0px 0px -64px 0px', threshold: 0.08 },
  )
  observer.observe(element.value)
  pending.value = true
})

onBeforeUnmount(() => {
  observer?.disconnect()
  preference?.removeEventListener('change', updatePreference)
})
</script>

<template>
  <component
    :is="as"
    ref="element"
    class="scroll-reveal"
    :class="[`reveal-${variant}`, { 'reveal-immediate': immediate }]"
    :data-reveal-state="pending ? 'pending' : 'visible'"
    :style="{ '--reveal-delay': `${delay}ms` }"
    @focusin="revealImmediately"
  >
    <slot />
  </component>
</template>

<style scoped>
.reveal-rise {
  opacity: 1;
  transform: translateY(0);
  transition:
    opacity var(--duration-reveal) var(--ease-out) var(--reveal-delay),
    transform var(--duration-reveal) var(--ease-out) var(--reveal-delay);
}
.reveal-rise[data-reveal-state='pending'] {
  opacity: 0;
  transform: translateY(var(--distance-reveal));
  transition: none;
}
.reveal-wipe :deep(img) {
  clip-path: inset(0);
  transform: none;
  transition:
    clip-path var(--duration-reveal) var(--ease-in-out) var(--reveal-delay),
    transform var(--duration-reveal) var(--ease-out) var(--reveal-delay);
}
.reveal-wipe[data-reveal-state='pending'] :deep(img) {
  clip-path: inset(0 50% 0 50%);
  transform: translateY(3%) scale(0.97);
  transition: none;
}
.reveal-immediate,
.reveal-immediate :deep(img) {
  transition: none;
}
@media (prefers-reduced-motion: reduce) {
  .scroll-reveal {
    opacity: 1;
    transform: none;
    clip-path: none;
    transition: none;
  }
  .scroll-reveal :deep(img) {
    clip-path: none;
    transform: none;
    transition: none;
  }
}
</style>
