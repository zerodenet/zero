import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');

function readUtf8(path) {
  return readFileSync(resolve(root, path), 'utf8');
}

function assertContains(path, text, message) {
  const content = readUtf8(path);
  if (!content.includes(text)) {
    throw new Error(`${path}: missing ${message}: ${text}`);
  }
}

function assertNotContains(path, text, message) {
  const content = readUtf8(path);
  if (content.includes(text)) {
    throw new Error(`${path}: unexpected ${message}: ${text}`);
  }
}

const source = 'project/tooling.md';
const html = '.vitepress/dist/project/tooling.html';

for (const path of [source, html]) {
  assertContains(path, '工程规则', 'UTF-8 Chinese heading');
  assertContains(path, 'status_api', 'current root feature');
  assertContains(path, 'event_dispatcher', 'current root feature');
  assertNotContains(path, '\u5bb8\u30e7\u25bc\u7459\u52eb\u57af', 'mojibake tooling heading');
  assertNotContains(path, 'inbound-socks5', 'removed legacy feature naming');
  assertNotContains(path, 'status-api', 'removed legacy feature naming');
}

console.log('docs encoding and tooling feature checks passed');
