import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://kitter.dev',
  i18n: {
    locales: ['en', 'zh'],
    defaultLocale: 'en',
    routing: { prefixDefaultLocale: false },
  },
  redirects: { '/en': '/' },
});
