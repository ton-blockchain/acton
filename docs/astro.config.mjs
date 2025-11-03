// @ts-check
import {defineConfig} from "astro/config"
import starlight from "@astrojs/starlight"
import starlightThemeRapide from "starlight-theme-rapide"
import {ExpressiveCodeTheme} from "@astrojs/starlight/expressive-code"
import fs from "node:fs"

import remarkHeadingId from "remark-custom-heading-id"

import {rehypeHeadingIds} from "@astrojs/markdown-remark"
import rehypeAutolinkHeadings from "rehype-autolink-headings"
import starlightLinksValidator from "starlight-links-validator"

import react from "@astrojs/react"

// https://astro.build/config
// https://starlight.astro.build/reference/configuration/
export default defineConfig({
  outDir: "./dist",
  site: "https://i582.github.io/",
  base: "/acton/",
  trailingSlash: "always",
  vite: {
    define: {
      global: "globalThis",
    },
    optimizeDeps: {
      include: ["buffer"],
      exclude: ["@ton/tolk-js", "@ton/sandbox"],
    },
    resolve: {
      alias: {
        buffer: "buffer",
      },
    },
    server: {
      fs: {
        allow: [".."],
      },
    },
  },
  markdown: {
    remarkPlugins: [remarkHeadingId],
    rehypePlugins: [
      rehypeHeadingIds,
      [
        rehypeAutolinkHeadings,
        {
          behavior: "append",
          properties: {
            class: "autolink-header",
            ariaHidden: "true",
            ariaLabel: "Link to this header",
            tabIndex: -1,
          },
        },
      ],
    ],
  },
  integrations: [
    starlight({
      title: {
        en: "Acton",
      },
      titleDelimiter: undefined,
      favicon: "/favicon.ico",
      logo: {
        dark: "/public/logo-dark.svg",
        light: "/public/logo-light.svg",
        alt: "Acton",
        replacesTitle: false,
      },
      // 'head' is auto-populated with SEO-friendly contents based on the page frontmatters
      head: [
        // {
        //     // Google tag (gtag.js)
        //     tag: "script",
        //     attrs: {
        //         async: true,
        //         src: "TODO: Add google analytics link",
        //     },
        // },
        // {
        //     // Per-page Google tag setup
        //     tag: "script",
        //     content:
        //         "window.dataLayer=window.dataLayer||[];function gtag(){dataLayer.push(arguments)}gtag('js',new Date());gtag('config','G-ZJ3GZHJ0Z5');",
        // },
      ],
      social: [
        {icon: "github", label: "GitHub", href: "https://github.com/i582/acton"},
        {icon: "telegram", label: "Telegram", href: "https://t.me/toncore"},
      ],
      editLink: {
        baseUrl: "https://github.com/i582/acton/edit/main/docs/src/content/docs/",
      },
      tableOfContents: {minHeadingLevel: 2, maxHeadingLevel: 4},
      expressiveCode: {
        themes: [
          "one-dark-pro",
          ExpressiveCodeTheme.fromJSONString(
            fs.readFileSync(new URL(`./themes/one-light-mod.jsonc`, import.meta.url), "utf-8"),
          ),
        ],
        useStarlightDarkModeSwitch: true,
        useStarlightUiThemeColors: true,
        shiki: {
          langs: [
            () => JSON.parse(fs.readFileSync("grammars/grammar-func.json", "utf-8")),
            () => JSON.parse(fs.readFileSync("grammars/grammar-tolk.json", "utf-8")),
            () => JSON.parse(fs.readFileSync("grammars/grammar-tlb.json", "utf-8")),
            () => JSON.parse(fs.readFileSync("grammars/grammar-tasm.json", "utf-8")),
          ],
        },
      },
      customCss: [
        // To adjust Starlight colors and styles
        "./src/starlight.custom.css",
      ],
      plugins: [
        starlightThemeRapide(),
        starlightLinksValidator({
          errorOnFallbackPages: false,
          errorOnInvalidHashes: false,
        }),
      ],
      credits: false,
      lastUpdated: true,
      disable404Route: false,
      // Note that UI translations are bundled by Starlight for many languages:
      // https://starlight.astro.build/guides/i18n/#translate-starlights-ui
      //
      // To use fallback content and translation notices provided by Starlight,
      // files across language folders must be named the same!
      defaultLocale: "root",
      locales: {
        root: {
          label: "English",
          lang: "en",
        },
      },
      sidebar: [
        {
          label: "Test Runner",
          items: [
            {
              label: "Tests",
              items: [
                {slug: "test-runner/tests/your-first-unit-test-in-tolk"},
                {slug: "test-runner/tests/your-first-integration-test-in-tolk"},
                {slug: "test-runner/tests/test-attributes"},
                {slug: "test-runner/tests/code-coverage"},
              ],
            },
          ],
        },
      ],
    }),
    react(),
  ],
  // redirects: {
  //   "/": "/spec/doc/book/continuations/basics-register-c0-cc-savelist-if-instruction/",
  //   "/book/continuations":
  //     "/spec/doc/book/continuations/basics-register-c0-cc-savelist-if-instruction/",
  // },
})
