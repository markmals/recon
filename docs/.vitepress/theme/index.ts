import type { Theme } from "vitepress";

import { themeContextKey, VoidZeroTheme } from "@voidzero-dev/vitepress-theme";
import footerBg from "@voidzero-dev/vitepress-theme/src/assets/vitest/footer-background.jpg";

import RecordHeader from "./components/RecordHeader.vue";
import RecordLayout from "./layouts/RecordLayout.vue";
import "./custom.css";
import "virtual:group-icons.css";

let wordmark = (fill: string) =>
    `data:image/svg+xml,${encodeURIComponent(
        `<svg xmlns="http://www.w3.org/2000/svg" width="92" height="24" viewBox="0 0 92 24"><text x="0" y="19" fill="${fill}" font-family="Arial, Helvetica, sans-serif" font-size="21" font-weight="700" letter-spacing="-0.5">Recon</text></svg>`,
    )}`;

export default {
    extends: VoidZeroTheme,
    Layout: RecordLayout,
    enhanceApp(ctx) {
        ctx.app.component("RecordHeader", RecordHeader);
        ctx.app.provide(themeContextKey, {
            logoDark: wordmark("#1d2631"),
            logoLight: wordmark("#f1f5f9"),
            logoAlt: "Recon",
            footerBg,
            monoIcon: wordmark("#f1f5f9"),
        });
    },
} satisfies Theme;
