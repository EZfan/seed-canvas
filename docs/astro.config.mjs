import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://ezfan.github.io',
  base: '/seed-canvas',
  integrations: [
    starlight({
      title: 'seed-canvas',
      description:
        'Seed is the Artwork. Open-source, self-hostable, deterministic generative-art platform.',
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/EZfan/seed-canvas',
        },
      ],
      sidebar: [
        {
          label: 'Guides',
          items: [
            { label: 'Quickstart', slug: 'guides/quickstart' },
            { label: 'CLI Reference', slug: 'guides/cli' },
            { label: 'Gallery Server', slug: 'guides/server' },
          ],
        },
        {
          label: 'Template Authors',
          items: [
            { label: 'Your First Template', slug: 'authors/first-template' },
            { label: 'Determinism Rules', slug: 'authors/determinism' },
          ],
        },
        {
          label: 'Self-Hosting',
          items: [{ label: 'Docker', slug: 'self-hosting/docker' }],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Seed Format', slug: 'reference/seed-format' },
            { label: 'Registry Index', slug: 'reference/registry' },
          ],
        },
      ],
    }),
  ],
});