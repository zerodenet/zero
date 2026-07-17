import { existsSync, readdirSync, readFileSync } from 'node:fs'
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

const GUIDE_ORDER = ['quickstart', 'gui-integration', 'config-failure-examples']

const PROTOCOL_ROOT_ORDER = ['index', 'configuration', 'incomplete']
const PROTOCOL_DETAIL_DIRS = ['socks5', 'http', 'mixed', 'vless', 'shadowsocks', 'trojan', 'hysteria2', 'mieru', 'vmess']

const EN_CN_TITLE_REPLACEMENTS: Array<[string, string]> = [
  ['inbound', '\u5165\u7ad9'],
  ['outbound', '\u51fa\u7ad9'],
  ['metadata', '\u5143\u6570\u636e'],
  ['shared', '\u5171\u4eab'],
  ['stream', '\u6d41'],
  ['crypto', '\u52a0\u5bc6'],
  ['udp', 'UDP'],
  ['connect', '\u8fde\u63a5'],
  ['configuration', '\u914d\u7f6e'],
  ['config', '\u914d\u7f6e'],
  ['connector', '\u8fde\u63a5\u5668'],
  ['roadmap', '\u8def\u7ebf\u56fe'],
  ['auth', '\u8ba4\u8bc1'],
  ['events', '\u4e8b\u4ef6'],
  ['event', '\u4e8b\u4ef6'],
  ['hooks', '\u94a9\u5b50'],
  ['service', '\u670d\u52a1'],
  ['provider', '\u63d0\u4f9b\u8005'],
  ['node', '\u8282\u70b9'],
  ['heartbeat', '\u5fc3\u8df3'],
  ['push', '\u63a8\u9001'],
  ['performance', '\u6027\u80fd'],
  ['limiting', '\u9650\u6d41'],
  ['rule', '\u89c4\u5219'],
  ['plan', '\u8ba1\u5212'],
  ['zero', 'Zero'],
  ['rule', '\u89c4\u5219'],
  ['failure', '\u6545\u969c'],
  ['examples', '\u793a\u4f8b'],
  ['lifecycle', '\u751f\u547d\u5468\u671f'],
]

const PROJECT_GROUPS: Record<string, string[]> = {
  '\u914d\u7f6e\u4e0e\u8fd0\u884c\u65f6': [
    'config',
    'modes-and-groups',
    'engine-plan',
    'api',
    'zero-rule-ir-v1',
  ],
  '\u67b6\u6784\u4e0e\u5b9e\u8df5': [
    'architecture',
    'features',
    'logging',
    'lifecycle',
    'control-plane',
    'positioning',
    'goals',
    'tooling',
    'panel-node-connector',
    'protocol-capabilities',
    'release-boundary',
    'zrs-0.1',
    'zrs-0.1-golden',
  ],
}

const CONTROL_PLANE_ORDER = [
  'index',
  '01-control-plane-roadmap',
  '02-api-endpoints',
  '03-http-adapter-design',
  '04-event-system',
  '05-auth-and-permissions',
  '06-service-provider-integration',
  '07-node-heartbeat-and-push',
  '08-performance-and-rate-limiting',
]

const CONTROL_PLANE_API_ORDER = [
  'index',
  'configuration',
  'http-api',
  'ipc-protocol',
  'events',
  'hooks',
  'push-connector',
  'cli',
  'contract',
]

const TESTING_ORDER = ['tun-e2e']

function isChineseText(text: string): boolean {
  return /[\u4e00-\u9fff]/.test(text)
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function localizeEnglishTitle(rawTitle: string): string {
  if (isChineseText(rawTitle)) {
    return rawTitle.trim()
  }

  let title = rawTitle.trim()
  for (const [en, zh] of EN_CN_TITLE_REPLACEMENTS) {
    const rule = new RegExp(`\\b${escapeRegExp(en)}\\b`, 'gi')
    title = title.replace(rule, zh)
  }

  return title
}

function toTitleCase(text: string): string {
  return text
    .replace(/\.md$/, '')
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .split(/[-_]/g)
    .filter(Boolean)
    .map((part) => `${part[0].toUpperCase()}${part.slice(1)}`)
    .join(' ')
    .trim()
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
    return localizeEnglishTitle(headingMatch[1].trim())
  }

  return localizeEnglishTitle(fallback)
}

function sidebarOverviewTextFromDirectory(dirPath: string, fallback: string): string | undefined {
  const readmePath = path.join(dirPath, README_FILE)
  if (existsSync(readmePath)) {
    return localizeEnglishTitle(sidebarTextFromFile(readmePath, fallback))
  }

  const indexPath = path.join(dirPath, INDEX_FILE)
  if (existsSync(indexPath)) {
    return localizeEnglishTitle(sidebarTextFromFile(indexPath, fallback))
  }

  return undefined
}

function entryNameFromLower(dirPath: string, lowerName: string): string | null {
  const found = readdirSync(dirPath).find((name) => name.toLowerCase() === lowerName)
  return found ?? null
}

function sidebarItemsFromOrderedFiles(
  dirName: string,
  baseRoute: string,
  orderedBasenames: string[],
  includeRemaining: boolean,
  used: Set<string> = new Set<string>(),
): SidebarItem[] {
  const dirPath = path.join(docsRoot, dirName)
  const fileNames = readdirSync(dirPath, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.md'))
    .map((entry) => entry.name.toLowerCase())

  const items: SidebarItem[] = []

  const pushFile = (basename: string) => {
    const lower = `${basename.toLowerCase()}.md`
    if (used.has(lower)) {
      return
    }
    if (!fileNames.includes(lower)) {
      return
    }
    if (lower === README_FILE.toLowerCase() || lower === INDEX_FILE.toLowerCase()) {
      return
    }

    const actual = entryNameFromLower(dirPath, lower)
    if (!actual) {
      return
    }
    const filePath = path.join(dirPath, actual)
    items.push({
      text: sidebarTextFromFile(filePath, toTitleCase(actual)),
      link: `${baseRoute}/${actual.replace(/\.md$/, '')}`,
    })
    used.add(lower)
  }

  for (const basename of orderedBasenames) {
    pushFile(basename)
  }

  if (includeRemaining) {
    const remaining = fileNames
      .filter((name) => name !== README_FILE.toLowerCase() && name !== INDEX_FILE.toLowerCase())
      .filter((name) => !used.has(name))
      .sort((a, b) => a.localeCompare(b))

    for (const remainingName of remaining) {
      const filePath = path.join(dirPath, remainingName)
      items.push({
        text: sidebarTextFromFile(filePath, toTitleCase(remainingName)),
        link: `${baseRoute}/${remainingName.replace(/\.md$/, '')}`,
      })
      used.add(remainingName)
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

function sidebarFromDirectory(dirName: string, baseRoute: string): SidebarItem[] {
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
        text:
          sidebarOverviewTextFromDirectory(path.join(dirPath, folder.name), toTitleCase(folder.name))
          ?? toTitleCase(folder.name),
        items: subItems,
      })
    }
  }

  const hasReadme = existsSync(path.join(dirPath, README_FILE))
  const hasIndex = existsSync(path.join(dirPath, INDEX_FILE))
  const hasOverview = hasReadme || hasIndex
  if (hasOverview) {
    const overview = sidebarOverviewTextFromDirectory(dirPath, toTitleCase(path.basename(dirName)))
    if (overview) {
      items.unshift({
        text: overview,
        link: `${baseRoute}/`,
      })
    }
  }

  return items
}

function sidebarFromDirectories(
  dirName: string,
  baseRoute: string,
  preferredDirOrder: string[] = [],
): SidebarItem[] {
  const dirPath = path.join(docsRoot, dirName)
  const folderEntries = readdirSync(dirPath, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith('.'))

  const folderNames = folderEntries.map((entry) => entry.name)
  const ordered = preferredDirOrder.filter((name) => folderNames.includes(name))
  const used = new Set(ordered)
  const rest = folderNames.filter((name) => !used.has(name)).sort((a, b) => a.localeCompare(b))
  const result: SidebarItem[] = []

  for (const folder of [...ordered, ...rest]) {
    const subItems = sidebarFromDirectory(
      path.join(dirName, folder),
      `${baseRoute}/${folder}`,
    )
    if (subItems.length > 0) {
      result.push({
        text:
          sidebarOverviewTextFromDirectory(path.join(dirPath, folder), toTitleCase(folder))
          ?? toTitleCase(folder),
        items: subItems,
      })
    }
  }

  return result
}

function sidebarProjectGroups(): SidebarItem[] {
  const used = new Set<string>()
  const items: SidebarItem[] = []

  for (const [groupText, files] of Object.entries(PROJECT_GROUPS)) {
    const groupItems = sidebarItemsFromOrderedFiles('project', '/project', files, true, used)
    if (groupItems.length > 0) {
      items.push({ text: groupText, items: groupItems })
    }
  }

  const otherItems = sidebarItemsFromOrderedFiles('project', '/project', [], true, used)
  if (otherItems.length > 0) {
    items.push({ text: '\u5176\u4ed6', items: otherItems })
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
      { text: '\u6307\u5357', link: '/guides/quickstart' },
      { text: '\u914d\u7f6e', link: '/project/config' },
      { text: '\u534f\u8bae', link: '/protocols/' },
      { text: '\u63a7\u5236\u5e73\u9762 API', link: '/control-plane-api/' },
      { text: '\u63a7\u5236\u5e73\u9762', link: '/control-plane/' },
      { text: '\u9879\u76ee', link: '/project/architecture' },
      { text: '\u6d4b\u8bd5', link: '/testing/tun-e2e' },
    ],

    sidebar: {
      '/guides/': [
        {
          text: '\u6307\u5357',
          items: sidebarItemsFromOrderedFiles('guides', '/guides', GUIDE_ORDER, true),
        },
      ],

      '/protocols/': [
        {
          text: '\u534f\u8bae\u603b\u89c8',
          items: sidebarItemsFromOrderedFiles('protocols', '/protocols', PROTOCOL_ROOT_ORDER, true),
        },
        {
          text: '\u534f\u8bae\u8be6\u7ec6',
          items: sidebarFromDirectories('protocols', '/protocols', PROTOCOL_DETAIL_DIRS),
        },
      ],

      '/project/': sidebarProjectGroups(),
      '/control-plane/': [
        {
          text: '\u63a7\u5236\u9762',
          items: sidebarItemsFromOrderedFiles('control-plane', '/control-plane', CONTROL_PLANE_ORDER, true),
        },
      ],

      '/control-plane-api/': [
        {
          text: '\u63a7\u5236\u5e73\u9762 API',
          items: sidebarItemsFromOrderedFiles('control-plane-api', '/control-plane-api', CONTROL_PLANE_API_ORDER, true),
        },
      ],

      '/testing/': [
        {
          text: '\u6d4b\u8bd5',
          items: sidebarItemsFromOrderedFiles('testing', '/testing', TESTING_ORDER, true),
        },
      ],
    },

    socialLinks: [{ icon: 'github', link: 'https://github.com/zerodenet/zero' }],

    search: {
      provider: 'local',
    },

    outline: {
      level: [2, 3],
      label: '\u672c\u9875\u5927\u7eb2',
    },

    docFooter: {
      prev: '\u4e0a\u4e00\u9875',
      next: '\u4e0b\u4e00\u9875',
    },

    lastUpdated: {
      text: '\u6700\u540e\u66f4\u65b0',
    },
  },
})
