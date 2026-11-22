<script setup lang="ts">
import { messages, repository } from '~/i18n'

const base = useRuntimeConfig().app.baseURL
const siteURL = 'https://iwe-code.ru/'

useHead(() => ({
  htmlAttrs: { lang: 'en' },
  title: messages.meta.title,
  link: [
    { rel: 'icon', href: `${base}icon.webp`, type: 'image/webp' },
    { rel: 'canonical', href: siteURL },
  ],
}))
useSeoMeta({
  description: () => messages.meta.description,
  ogTitle: () => messages.meta.title,
  ogDescription: () => messages.meta.description,
  ogType: 'website',
  ogUrl: siteURL,
  ogImage: `${siteURL}images/workspace-en.webp`,
  twitterCard: 'summary_large_image',
})
</script>

<template>
  <div class="landing">
    <SiteHeader :messages="messages" />
    <main id="main" class="shell">
      <section class="hero" aria-labelledby="hero-title">
        <a class="intro-link" href="#download"
          ><span class="intro-dot" />{{ messages.hero.release }}<SiteIcon name="arrow"
        /></a>
        <h1 id="hero-title" class="preserve-lines">{{ messages.hero.title }}</h1>
        <div class="hero-bottom">
          <p class="hero-description">{{ messages.hero.description }}</p>
          <div class="hero-actions">
            <a class="button" href="#download"
              >{{ messages.hero.download }}<SiteIcon name="download" /></a
            ><a class="text-link" :href="repository"
              >{{ messages.hero.source }}<SiteIcon name="arrow"
            /></a>
          </div>
        </div>
        <p class="platform-note">{{ messages.hero.platforms }}</p>
        <ProductShowcase :messages="messages" />
      </section>

      <FeatureSection :messages="messages" />

      <section id="workflow" class="workflow section-space" aria-labelledby="workflow-title">
        <ScrollReveal class="workflow-copy">
          <h2 id="workflow-title" class="section-title preserve-lines">
            {{ messages.workflow.title }}
          </h2>
          <p class="section-description">{{ messages.workflow.description }}</p>
        </ScrollReveal>
        <div class="workflow-content">
          <ScrollReveal as="figure" class="comparison-figure" variant="wipe">
            <img
              :src="`${base}images/comparison-en.webp`"
              :alt="messages.preview.altComparison"
              width="2880"
              height="1840"
              loading="lazy"
            />
            <figcaption>{{ messages.workflow.caption }}</figcaption>
          </ScrollReveal>
          <div class="workflow-points">
            <ScrollReveal
              v-for="(item, index) in messages.workflow.items"
              :key="item.title"
              as="article"
              :delay="index * 60"
            >
              <h3>{{ item.title }}</h3>
              <p>{{ item.description }}</p>
            </ScrollReveal>
          </div>
        </div>
      </section>

      <section class="local-section section-space" aria-labelledby="local-title">
        <ScrollReveal class="local-copy">
          <h2 id="local-title" class="section-title preserve-lines">{{ messages.local.title }}</h2>
          <p class="section-description">{{ messages.local.description }}</p>
          <ul class="local-points">
            <li v-for="point in messages.local.points" :key="point">
              <SiteIcon name="check" />{{ point }}
            </li>
          </ul>
        </ScrollReveal>
        <ScrollReveal class="local-diagram" variant="scene">
          <div class="diagram-computer">
            <SiteIcon name="computer" /><span>{{ messages.local.folderLabel }}</span
            ><span class="local-indicator" />
          </div>
          <div class="diagram-connection" />
          <div class="diagram-workspace">
            <div class="diagram-title">
              <img :src="`${base}icon.webp`" width="24" height="24" alt="" /><span>{{
                messages.local.spaceLabel
              }}</span>
            </div>
            <div class="diagram-row">
              <SiteIcon name="folder" />{{ messages.local.projectLabel
              }}<span class="diagram-line" />
            </div>
            <div class="diagram-row">
              <SiteIcon name="code" />{{ messages.local.codeLabel }}<span class="diagram-line" />
            </div>
            <div class="diagram-row">
              <SiteIcon name="branch" />{{ messages.local.historyLabel
              }}<span class="diagram-line" />
            </div>
          </div>
          <p>{{ messages.local.note }}</p>
        </ScrollReveal>
      </section>

      <RoadmapSection :messages="messages" preview />
      <DownloadSection :messages="messages" />

      <section class="faq section-space" aria-labelledby="faq-title">
        <h2 id="faq-title">{{ messages.faq.title }}</h2>
        <div class="faq-list">
          <details v-for="item in messages.faq.items" :key="item.question">
            <summary>{{ item.question }}<SiteIcon name="plus" /></summary>
            <p>{{ item.answer }}</p>
          </details>
        </div>
      </section>
    </main>
    <SiteFooter :messages="messages" />
  </div>
</template>

<style scoped>
@supports (animation-timeline: scroll()) {
  @media (prefers-reduced-motion: no-preference) {
    .landing::before {
      content: '';
      position: fixed;
      inset: 0 0 auto;
      z-index: 20;
      height: 2px;
      background: var(--accent);
      pointer-events: none;
      transform-origin: left;
      animation: reading-progress auto linear both;
      animation-timeline: scroll(root block);
    }
  }
}
@keyframes reading-progress {
  from {
    transform: scaleX(0);
  }
  to {
    transform: scaleX(1);
  }
}
.hero {
  padding-top: 80px;
}
.intro-link {
  display: inline-flex;
  gap: 9px;
  align-items: center;
  padding: 6px 10px 6px 8px;
  border: 1px solid var(--line);
  border-radius: 5px;
  color: var(--muted);
  font-size: 11px;
}
.intro-link svg {
  width: 12px;
  height: 12px;
  margin-left: 7px;
}
.intro-dot {
  height: 5px;
  width: 5px;
  border-radius: 50%;
  background: var(--accent);
}
.hero h1 {
  font-size: clamp(54px, 6.4vw, 88px);
  font-weight: 520;
  letter-spacing: -0.062em;
  line-height: 1.04;
  margin: 30px 0;
}
.hero-bottom {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 36px;
}
.hero-description {
  max-width: 475px;
  color: var(--muted);
  font-size: 17px;
  line-height: 1.6;
  margin: 0;
}
.hero-actions {
  display: flex;
  align-items: center;
  gap: 24px;
  padding-bottom: 4px;
}
.hero-actions .button {
  white-space: nowrap;
}
.platform-note {
  color: var(--quiet);
  font-size: 11px;
  margin: 20px 0 0;
}
.workflow-points h3 {
  font-size: 16px;
  font-weight: 550;
  margin: 0 0 13px;
}
.workflow-points p {
  font-size: 14px;
  line-height: 1.65;
  color: var(--muted);
  margin: 0;
}
.workflow {
  border-top: 1px solid var(--line);
}
.workflow-copy {
  display: flex;
  justify-content: space-between;
  gap: 50px;
  align-items: flex-end;
}
.workflow-copy .section-description {
  max-width: 380px;
  margin: 0 0 5px;
}
.workflow-content {
  margin-top: 55px;
  display: grid;
  grid-template-columns: 1.9fr 1fr;
  gap: 70px;
  align-items: center;
}
.comparison-figure {
  margin: 0;
  min-width: 0;
}
.comparison-figure img {
  border: 1px solid var(--line-strong);
  border-radius: 6px;
  width: 100%;
  height: auto;
}
.comparison-figure figcaption {
  font-size: 11px;
  color: var(--muted);
  margin-top: 16px;
}
.workflow-points {
  display: grid;
  gap: 30px;
}
.local-section {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 100px;
  border-top: 1px solid var(--line);
  align-items: center;
}
.local-copy .section-description {
  max-width: 440px;
}
.local-points {
  list-style: none;
  padding: 0;
  margin: 28px 0 0;
  display: grid;
  gap: 16px;
  font-size: 13px;
}
.local-points li {
  display: flex;
  align-items: center;
  gap: 10px;
}
.local-points svg {
  width: 14px;
  height: 14px;
  color: var(--accent);
  flex-shrink: 0;
}
.local-diagram {
  max-width: 390px;
  width: 100%;
  margin-inline: auto;
  padding: 20px 10px;
}
.diagram-computer {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 17px 20px;
  font-size: 13px;
  border: 1px solid var(--line-strong);
  border-radius: 8px;
}
.diagram-computer svg {
  width: 20px;
  height: 20px;
  color: var(--muted);
}
.local-indicator {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--accent);
  margin-left: auto;
}
.diagram-connection {
  width: 1px;
  height: 32px;
  background: var(--accent);
  margin: auto;
}
.diagram-workspace {
  padding: 20px;
  background: var(--surface);
  border: 1px solid var(--line-strong);
  border-radius: 8px;
}
.diagram-title {
  display: flex;
  align-items: center;
  gap: 9px;
  padding-bottom: 22px;
  margin-bottom: 12px;
  border-bottom: 1px solid var(--line);
  font-size: 13px;
}
.diagram-row {
  display: flex;
  align-items: center;
  gap: 13px;
  font-size: 12px;
  color: var(--muted);
  padding: 12px 0;
}
.diagram-row svg {
  width: 17px;
  height: 17px;
}
.diagram-line {
  height: 4px;
  max-width: 60px;
  flex: 1;
  margin-left: auto;
  background: var(--line);
  border-radius: 2px;
}
.local-diagram > p {
  font-size: 11px;
  line-height: 1.6;
  text-align: center;
  color: var(--quiet);
  max-width: 270px;
  margin: 22px auto 0;
}
@media (prefers-reduced-motion: no-preference) {
  .local-diagram[data-reveal-state='visible']
    :is(.diagram-computer, .diagram-workspace, .diagram-row) {
    transition:
      opacity var(--duration-reveal) var(--ease-out) var(--diagram-delay, 0ms),
      transform var(--duration-reveal) var(--ease-out) var(--diagram-delay, 0ms);
  }
  .local-diagram[data-reveal-state='pending']
    :is(.diagram-computer, .diagram-workspace, .diagram-row) {
    opacity: 0;
    transform: translateY(18px);
  }
  .diagram-connection {
    transform-origin: top;
  }
  .local-diagram[data-reveal-state='pending'] .diagram-connection {
    transform: scaleY(0);
  }
  .local-diagram[data-reveal-state='visible'] .diagram-connection {
    transition: transform 320ms var(--ease-in-out) 80ms;
  }
  .diagram-workspace {
    --diagram-delay: 230ms;
  }
  .diagram-row:nth-of-type(2) {
    --diagram-delay: 320ms;
  }
  .diagram-row:nth-of-type(3) {
    --diagram-delay: 380ms;
  }
  .diagram-row:nth-of-type(4) {
    --diagram-delay: 440ms;
  }
}
.faq {
  display: grid;
  grid-template-columns: 1fr 1.5fr;
  gap: 60px;
  padding-top: 0;
}
.faq h2 {
  margin: 5px 0 0;
  font-size: 27px;
  font-weight: 500;
  letter-spacing: -0.035em;
}
.faq-list details {
  border-bottom: 1px solid var(--line);
  padding: 22px 0;
}
.faq-list details:first-child {
  padding-top: 0;
}
.faq-list summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  cursor: pointer;
  font-size: 14px;
  list-style: none;
}
.faq-list summary::-webkit-details-marker {
  display: none;
}
.faq-list summary svg {
  width: 17px;
  height: 17px;
  flex-shrink: 0;
  color: var(--muted);
}
.faq-list details[open] summary svg {
  transform: rotate(45deg);
}
.faq-list p {
  color: var(--muted);
  font-size: 13px;
  line-height: 1.7;
  margin: 16px 28px 0 0;
}
@media (max-width: 1000px) {
  .hero-bottom {
    align-items: flex-start;
    flex-direction: column;
    gap: 25px;
  }
  .hero-actions {
    gap: 20px;
  }
  .hero-description {
    max-width: 550px;
  }
  .workflow-content {
    gap: 36px;
    grid-template-columns: 1.4fr 1fr;
  }
  .local-section {
    gap: 40px;
  }
}
@media (max-width: 700px) {
  .hero {
    padding-top: 52px;
  }
  .hero h1 {
    font-size: clamp(43px, 10vw, 62px);
    letter-spacing: -0.055em;
  }
  .hero-description {
    font-size: 15px;
  }
  .hero-actions {
    flex-wrap: wrap;
    gap: 20px;
  }
  .workflow-copy {
    display: block;
  }
  .workflow-copy .section-description {
    margin: 22px 0 0;
    max-width: 450px;
  }
  .workflow-content {
    grid-template-columns: 1fr;
    margin-top: 34px;
  }
  .workflow-points {
    gap: 24px;
  }
  .local-section {
    grid-template-columns: 1fr;
    gap: 32px;
  }
  .local-diagram {
    max-width: 400px;
  }
  .faq {
    grid-template-columns: 1fr;
    gap: 36px;
  }
}
</style>
