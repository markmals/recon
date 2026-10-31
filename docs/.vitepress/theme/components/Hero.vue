<template>
    <main class="recon-hero">
        <div class="recon-hero__inner">
            <div class="recon-hero__intro">
                <h1>Recon</h1>
                <p class="recon-hero__lead">Evidence-led reverse engineering for apps.</p>
                <p class="recon-hero__description">
                    An agent-facing workbench for understanding software through binaries, runtime
                    objects, native views, web documents and styles, network activity, resources,
                    and platform APIs, with macOS as the first host.
                </p>
                <p class="recon-hero__description">
                    Available today: macOS app discovery, metadata, and launch; window listing,
                    accessibility trees, and screenshots; and native AppKit view, layer, layout, and
                    font inspection through an explicit session. Other inspectors and the behavioral
                    investigation loop remain future work.
                </p>
                <a :href="visionLink" class="recon-hero__action">Read the vision</a>
            </div>
            <section class="recon-hero__loop" aria-labelledby="investigation-loop">
                <h2 id="investigation-loop">A planned workflow: behavioral investigation</h2>
                <ol>
                    <li>
                        <h3>Discover</h3>
                        <p>Choose a behavior and record what remains unknown.</p>
                    </li>
                    <li>
                        <h3>Experiment</h3>
                        <p>Vary one condition against a recorded baseline.</p>
                    </li>
                    <li>
                        <h3>Specify</h3>
                        <p>State the behavior the evidence supports.</p>
                    </li>
                    <li>
                        <h3>Compare</h3>
                        <p>
                            Check another implementation against the reference and keep differences
                            visible.
                        </p>
                    </li>
                </ol>
            </section>
        </div>
    </main>
</template>

<script setup lang="ts">
import { withBase } from "vitepress";

let visionLink = withBase("/VISION");
</script>

<style scoped>
.recon-hero {
    border-bottom: 1px solid var(--vp-c-divider);
}

.recon-hero__inner {
    display: grid;
    max-width: 80rem;
    margin: 0 auto;
}

.recon-hero__intro {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    gap: 1.5rem;
    padding: 4rem 1.5rem;
}

.recon-hero__intro h1 {
    margin: 0;
    color: inherit;
    font-size: clamp(3.5rem, 8vw, 6rem);
    line-height: 1;
    letter-spacing: -0.04em;
}

.recon-hero__lead {
    max-width: 16ch;
    margin: 0;
    color: inherit;
    font-size: clamp(1.5rem, 3vw, 2.25rem);
    line-height: 1.22;
    font-weight: 600;
}

.recon-hero__description {
    max-width: 62ch;
    margin: 0;
    color: color-mix(in srgb, currentColor 78%, transparent);
    font-size: 1rem;
    line-height: 1.65;
}

.recon-hero__action {
    display: inline-flex;
    align-items: center;
    min-height: 2.75rem;
    padding: 0.625rem 1.25rem;
    border: 1px solid var(--vp-c-brand-1);
    border-radius: 0.375rem;
    background: var(--vp-c-brand-1);
    color: var(--vp-c-bg);
    font-weight: 600;
    text-decoration: none;
    transition: background-color 0.15s ease;
}

.recon-hero__action:hover {
    background: color-mix(in srgb, var(--vp-c-brand-1) 85%, var(--vp-c-bg));
    color: var(--vp-c-bg);
}

.recon-hero__action:focus-visible {
    outline: 2px solid var(--vp-c-brand-1);
    outline-offset: 3px;
}

.recon-hero__loop {
    padding: 2.5rem 1.5rem 4rem;
    border-top: 1px solid var(--vp-c-divider);
}

.recon-hero__loop h2 {
    margin: 0 0 1.5rem;
    color: inherit;
    font-size: 1.125rem;
    font-weight: 600;
}

.recon-hero__loop ol {
    display: grid;
    gap: 0;
    margin: 0;
    padding-left: 1.5rem;
}

.recon-hero__loop li {
    padding: 1rem 0 1rem 0.25rem;
    border-top: 1px solid var(--vp-c-divider);
    color: inherit;
}

.recon-hero__loop li::marker {
    color: var(--vp-c-brand-1);
    font-family: var(--vp-font-family-mono);
}

.recon-hero__loop h3 {
    margin: 0;
    color: inherit;
    font-size: 1rem;
    font-weight: 600;
}

.recon-hero__loop p {
    max-width: 40ch;
    margin: 0.375rem 0 0;
    color: color-mix(in srgb, currentColor 78%, transparent);
    line-height: 1.5;
}

@media (min-width: 860px) {
    .recon-hero__inner {
        grid-template-columns: minmax(0, 1.25fr) minmax(18rem, 0.75fr);
        min-height: 36rem;
    }

    .recon-hero__intro {
        padding: 5rem 4rem 5rem 3rem;
    }

    .recon-hero__loop {
        align-content: center;
        padding: 4rem 3rem;
        border-top: 0;
        border-left: 1px solid var(--vp-c-divider);
    }
}
</style>
