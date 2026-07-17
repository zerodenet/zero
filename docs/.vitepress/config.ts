import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import { defineConfig } from 'vitepress'

const docsRoot = path.resolve(fileURLToPath(new URL('..', import.meta.url)))
const README_FILE = 'README.md'
const INDEX_FILE = 'index.md'

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

function sidebarOverviewTextFromDirectory(dirPath: string, fallback: string): string | undefined {
  const readmePath = path.join(dirPath, README_FILE)
  if (existsSync(readmePath)) {
    return sidebarTextFromFile(readmePath, fallback)
  }

  const indexPath = path.join(dirPath, INDEX_FILE)
  if (existsSync(indexPath)) {
    return sidebarTextFromFile(indexPath, fallback)
  }

  return undefined
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
    const fileName = page.name.toLowerCase()
    if (fileName === README_FILE.toLowerCase() || fileName === INDEX_FILE.toLowerCase()) {
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
        text: sidebarOverviewTextFromDirectory(path.join(dirPath, folder.name), toTitleCase(folder.name)) ?? toTitleCase(folder.name),
        items: subItems,
      })
    }
  }

  const overview = sidebarOverviewTextFromDirectory(dirPath, toTitleCase(path.basename(dirName)))
  if (overview) {
    items.unshift({
      text: overview,
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
      { text: '指南', link: '/guides/quickstart' },
      { text: '配置', link: '/project/config' },
      { text: '协议', link: '/protocols/' },
      { text: '控制平面 API', link: '/control-plane-api/' },
      { text: '控制面', link: '/control-plane/' },
      { text: '项目', link: '/project/architecture' },
      { text: '测试', link: '/testing/tun-e2e' },
    ],

    sidebar: {
      '/guides/': [{ text: '指南', items: sidebarFromDirectory('guides', '/guides') }],
      '/protocols/': [{ text: '协议', items: sidebarFromDirectory('protocols', '/protocols') }],

      '/project/': sidebarFromDirectory('project', '/project'),
      '/control-plane/': sidebarFromDirectory('control-plane', '/control-plane'),
      '/testing/': sidebarFromDirectory('testing', '/testing'),

      '/control-plane-api/': [{ text: '控制平面 API', items: sidebarFromDirectory('control-plane-api', '/control-plane-api') }],
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
