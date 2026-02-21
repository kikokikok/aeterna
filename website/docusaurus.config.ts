import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'Aeterna',
  tagline: 'Universal Memory & Knowledge Framework for AI Agents',
  favicon: 'img/favicon.ico',
  url: 'https://kikokikok.github.io',
  baseUrl: '/aeterna/',
  organizationName: 'kikokikok',
  projectName: 'aeterna',
  onBrokenLinks: 'warn',
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },
  themes: ['@docusaurus/theme-mermaid'],
  markdown: {
    mermaid: true,
    format: 'detect',
  },
  plugins: [
    function pluginResolveSymlinks() {
      return {
        name: 'docusaurus-plugin-resolve-symlinks',
        configureWebpack() {
          return {
            resolve: {
              symlinks: false,
            },
          };
        },
      };
    },
  ],
  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          routeBasePath: 'docs',
          editUrl:
            'https://github.com/kikokikok/aeterna/tree/main/website/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    navbar: {
      title: 'Aeterna',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docs',
          position: 'left',
          label: 'Documentation',
        },
        {
          href: 'https://github.com/kikokikok/aeterna',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            {
              label: 'Getting Started',
              to: '/docs/',
            },
          ],
        },
        {
          title: 'Community',
          items: [
            {
              label: 'GitHub',
              href: 'https://github.com/kikokikok/aeterna',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Aeterna Contributors.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'yaml', 'bash', 'json'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
