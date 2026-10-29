import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://kitter.dev',
  i18n: {
    locales: ['zh', 'en'],
    defaultLocale: 'zh',
    routing: { prefixDefaultLocale: false },
  },
});
