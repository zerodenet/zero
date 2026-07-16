import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, extname, join, relative, resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const ignoredDirectories = new Set(['.vitepress', 'node_modules']);

function readUtf8(path) {
  return readFileSync(resolve(root, path), 'utf8');
}

function walk(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (entry.isDirectory() && ignoredDirectories.has(entry.name)) {
      return [];
    }
    const path = join(directory, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

function assertContains(path, text, message) {
  const content = readUtf8(path);
  if (!content.includes(text)) {
    throw new Error(`${path}: 缺少${message}: ${text}`);
  }
}

function assertNotContains(path, text, message) {
  const content = readUtf8(path);
  if (content.includes(text)) {
    throw new Error(`${path}: 包含不应出现的${message}: ${text}`);
  }
}

function localTargetExists(source, rawTarget) {
  const withoutAnchor = rawTarget.split('#')[0].split('?')[0];
  if (!withoutAnchor) {
    return true;
  }

  const target = withoutAnchor.startsWith('/')
    ? join(root, withoutAnchor)
    : resolve(dirname(source), withoutAnchor);

  if (existsSync(target)) {
    return true;
  }
  if (!extname(target) && existsSync(`${target}.md`)) {
    return true;
  }
  return !extname(target) && existsSync(join(target, 'index.md'));
}

const markdownFiles = walk(root).filter((path) => path.endsWith('.md'));
const mojibakePatterns = ['锛', '銆', '鈥', '馃', '\uFFFD', 'Ã', 'Â'];
const brokenLinks = [];

for (const file of markdownFiles) {
  const content = readFileSync(file, 'utf8');
  const displayPath = relative(root, file);

  if (!content.trimStart().startsWith('#') && !content.trimStart().startsWith('---')) {
    throw new Error(`${displayPath}: 文档必须以标题或 frontmatter 开头`);
  }

  for (const pattern of mojibakePatterns) {
    if (content.includes(pattern)) {
      throw new Error(`${displayPath}: 检测到疑似乱码: ${pattern}`);
    }
  }

  const links = content.matchAll(/\[[^\]]*]\(([^)]+)\)/g);
  for (const match of links) {
    const target = match[1].trim().replace(/^<|>$/g, '');
    if (!target || /^(https?:|mailto:|#)/.test(target)) {
      continue;
    }
    if (!localTargetExists(file, target)) {
      brokenLinks.push(`${displayPath}: ${target}`);
    }
  }
}

if (brokenLinks.length > 0) {
  throw new Error(`发现失效的本地链接:\n${brokenLinks.join('\n')}`);
}

const source = 'project/tooling.md';
const html = '.vitepress/dist/project/tooling.html';

for (const path of [source, html]) {
  assertContains(path, '工程规则', 'UTF-8 中文标题');
  assertContains(path, 'status_api', '当前根 feature');
  assertContains(path, 'event_dispatcher', '当前根 feature');
  assertNotContains(path, '\u5bb8\u30e7\u25bc\u7459\u52eb\u57af', '乱码标题');
  assertNotContains(path, 'inbound-socks5', '已移除的旧 feature 名');
  assertNotContains(path, 'status-api', '已移除的旧 feature 名');
}

assertContains('README.md', 'control-plane-api/', '正式控制面文档入口');
assertContains('README.md', '历史设计与方案背景', '历史控制面目录说明');
assertContains('protocols/index.md', '协议概览', '中文协议总览');
assertNotContains('.vitepress/config.ts', '/protocols/http-connect/', '失效的 HTTP CONNECT 导航');

console.log(`文档检查通过：${markdownFiles.length} 个 Markdown 文件，编码和本地链接正常`);
