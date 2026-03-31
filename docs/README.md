# Documentation Source (Nuxt + i18n)

This directory is the source of a Nuxt-based multilingual documentation site, aligned with tauri-vuetify-starter docs layout.

## Structure

- `src/pages/`: Nuxt pages (`index.vue`, `[...slug].vue`)
- `src/app.vue`: Nuxt app entry
- `content/en|fr|ja|ko|zhHans|zhHant/`: locale documents
- `nuxt.config.ts`: Nuxt and i18n settings
- `content.config.ts`: Nuxt Content collections by locale
- `tsconfig.json`: Nuxt project references

## Local Development

```bash
pnpm -C docs install
pnpm -C docs dev
```

## Build

```bash
pnpm -C docs build
pnpm -C docs generate
```

## Content Entry Points

- English top page: `content/en/index.md`
- Japanese top page: `content/ja/index.md`
- French top page: `content/fr/index.md`
- Korean top page: `content/ko/index.md`
- Simplified Chinese top page: `content/zhHans/index.md`
- Traditional Chinese top page: `content/zhHant/index.md`
