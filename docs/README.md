# Acton Docs

This directory contains the documentation site for Acton. It is a static
[Next.js](https://nextjs.org/) application backed by
[Fumadocs](https://fumadocs.dev/) and MDX content, with a custom landing page,
docs navigation, local search, Open Graph image generation, and syntax
highlighting for Acton and TON-related languages.

## What lives here

- `content/docs/`: end-user documentation pages written in MDX.
- `content/docs/**/meta.json`: sidebar structure and section metadata.
- `src/app/`: Next.js routes for the landing page, docs pages, search endpoint,
  and generated OG images.
- `src/components/`: reusable UI for the landing page and docs experience.
- `src/lib/`: content loading, source mapping, and docs helpers.
- `grammars/`: custom Shiki grammars used for highlighted code blocks.
- `source.config.ts`: Fumadocs and MDX configuration for the docs collection.

## Local development

This workspace uses Bun.

```bash
bun install
bun run dev
```

Open `http://localhost:3000` for the landing page and
`http://localhost:3000/docs/welcome` for the documentation entry point.

## Available scripts

- `bun run dev`: start the local development server.
- `bun run build`: produce the static production build.
- `bun run start`: serve the production build locally.
- `bun run lint`: run ESLint for the docs app.
- `bun run lint:links`: validate internal and external MDX links.
- `bun run lint:links:internal`: validate internal MDX links without network requests.
- `bun run lint:links:external`: validate external MDX links and fail on redirects.

Production deployment is handled by CI via `.github/workflows/deploy-docs.yml`.

## Netlify deploy previews

This repo also includes a root-level `netlify.toml` for Netlify Deploy
Previews of the docs app. The Netlify build is scoped to `docs/`, publishes
`docs/out`, and skips non-PR contexts so GitHub Pages remains the production
host.

To enable previews:

1. Link the repository to a Netlify site with continuous deployment enabled.
2. Keep Deploy Previews enabled for pull requests in the Netlify site settings.
3. Leave the build settings managed by `netlify.toml`.

With this setup, Netlify will post a Deploy Preview for pull requests that
change files under `docs/`.

## Editing content

Most docs changes happen under `content/docs/`. When you add or move pages,
update the nearby `meta.json` so navigation stays correct. For richer content,
reuse the shared MDX components and docs UI instead of embedding one-off markup
directly into pages.

Some docs trees are generated and should not be edited by hand. Their
source-of-truth inputs live outside `docs/`:

- `src/doc/man/*.md` -> command docs, terminal help text, and manpages
- `lib/` -> `content/docs/standard_library`
- `crates/tolk-compiler/assets/tolk-stdlib/` -> `content/docs/tolk_standard_library`
- linter rule metadata -> `content/docs/rules`

After changing those inputs, rerun:

```bash
cargo run --bin acton -- docgen
```
