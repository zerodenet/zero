import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import { defineConfig } from 'vitepress'

const docsRoot = path.resolve(fileURLToPath(new URL('..', import.meta.url)))
const README_FILE = 'README.md'

type SidebarItem = {
  text: string
  items?: SidebarItem[]
  link?: string
}

function toTitleCase(text: string): string {
  return text
    .replace(/\.md$/, '')
    .split(/[-_]/g)
    .filter(Boolean)
    .map((part) => `${part[0].toUpperCase()}${part.slice(1)}`)
    .join(' ')
}

function sidebarTextFromFile(filePath: string, fallback: string): string {
  const text = readFileSync(filePath, 'utf8')

  const frontMatterMatch = text.match(/^---\r?\n([\s\S]*?)\r?\n---/)
  if (frontMatterMatch) {
    const titleMatch = frontMatterMatch[1].match(/^\s*title:\s*(.+)\s*$/m)
    if (titleMatch?.[1]) {
      return titleMatch[1].trim().replace(/^["']|["']$/g, '')
    }
  }

  const headingMatch = text.match(/^\s*#\s+(.+)$/m)
  if (headingMatch?.[1]) {
    return headingMatch[1].trim()
  }

  return fallback
}

function sidebarFromDirectory(
  dirName: string,
  baseRoute: string,
): SidebarItem[] {
  const dirPath = path.join(docsRoot, dirName)
  const entries = readdirSync(dirPath, { withFileTypes: true })
  const folders = entries
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith('.'))
    .sort((a, b) => a.name.localeCompare(b.name))
  const pages = entries
    .filter((entry) => entry.isFile() && entry.name.endsWith('.md'))
    .sort((a, b) => a.name.localeCompare(b.name))

  const items: SidebarItem[] = []

  for (const page of pages) {
    if (page.name.toLowerCase() === README_FILE.toLowerCase()) {
      continue
    }

    const filePath = path.join(dirPath, page.name)
    items.push({
      text: sidebarTextFromFile(filePath, toTitleCase(page.name)),
      link: `${baseRoute}/${page.name.replace(/\.md$/, '')}`,
    })
  }

  for (const folder of folders) {
    const subItems = sidebarFromDirectory(
      path.join(dirName, folder.name),
      `${baseRoute}/${folder.name}`,
    )
    if (subItems.length > 0) {
      items.push({
        text: toTitleCase(folder.name),
        items: subItems,
      })
    }
  }

  const hasReadme = existsSync(path.join(dirPath, README_FILE))

  if (hasReadme) {
    items.unshift({
      text: 'Overview',
      link: `${baseRoute}/`,
    })
  }

  return items
}

export default defineConfig({
  title: 'Zero',
  description: 'Zero - Rust modular proxy toolkit',
  lang: 'zh-CN',
  lastUpdated: true,

  themeConfig: {
    nav: [
      { text: 'Guides', link: '/guides/quickstart' },
      { text: 'Config', link: '/project/config' },
      { text: 'Protocols', link: '/protocols/' },
      { text: 'Control API', link: '/control-plane-api/' },
      { text: 'Control Plane', link: '/control-plane/' },
      { text: 'Project', link: '/project/architecture' },
      { text: 'Testing', link: '/testing/tun-e2e' },
    ],

    sidebar: {
      '/guides/': [
        {
          text: 'Guides',
          items: [
            { text: 'Quickstart', link: '/guides/quickstart' },
            { text: 'GUI Integration', link: '/guides/gui-integration' },
            { text: 'Config Failure Examples', link: '/guides/config-failure-examples' },
          ],
        },
      ],

      '/protocols/': [
        {
          text: 'Protocols Overview',
          items: [
            { text: 'Overview', link: '/protocols/' },
            { text: 'Configuration', link: '/protocols/configuration' },
            { text: 'Incomplete', link: '/protocols/incomplete' },
          ],
        },
        {
          text: 'Protocol Details',
          items: [
            { text: 'SOCKS5', link: '/protocols/socks5/' },
            { text: 'HTTP CONNECT', link: '/protocols/http/' },
            { text: 'Mixed', link: '/protocols/mixed/' },
            { text: 'VLESS', link: '/protocols/vless/' },
            { text: 'Shadowsocks', link: '/protocols/shadowsocks/' },
            { text: 'Trojan', link: '/protocols/trojan/' },
            { text: 'Hysteria2', link: '/protocols/hysteria2/' },
            { text: 'Mieru', link: '/protocols/mieru/' },
            { text: 'VMess', link: '/protocols/vmess/' },
          ],
        },
      ],

      '/project/': sidebarFromDirectory('project', '/project'),
      '/control-plane/': sidebarFromDirectory('control-plane', '/control-plane'),
      '/testing/': sidebarFromDirectory('testing', '/testing'),

      '/control-plane-api/': [
        {
          text: 'Control API',
          items: [
            { text: 'Overview', link: '/control-plane-api/' },
            { text: 'Configuration', link: '/control-plane-api/configuration' },
            { text: 'HTTP API', link: '/control-plane-api/http-api' },
            { text: 'IPC Protocol', link: '/control-plane-api/ipc-protocol' },
            { text: 'Events', link: '/control-plane-api/events' },
            { text: 'Hooks', link: '/control-plane-api/hooks' },
            { text: 'Push Connector', link: '/control-plane-api/push-connector' },
            { text: 'CLI', link: '/control-plane-api/cli' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/zerodenet/zero' },
    ],

    search: {
      provider: 'local',
    },

    outline: {
      level: [2, 3],
      label: 'On this page',
    },

    docFooter: {
      prev: 'Prev',
      next: 'Next',
    },

    lastUpdated: {
      text: 'Last Updated',
    },
  },
})
