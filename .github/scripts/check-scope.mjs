import { execFileSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const unreadDirectories = ['docs/', 'assets/screenshots/', '.agents/', '.github/ISSUE_TEMPLATE/', 'packaging/'];
const unreadFiles = ['LICENSE', 'circadia-harness/README.md', 'circadia-bindings/README.md'];

export function selectChecks(paths, full = false) {
  let rust = full;
  for (const path of paths) {
    const topLevelGuide = !path.includes('/') && path.endsWith('.md');
    if (topLevelGuide || unreadFiles.includes(path) || unreadDirectories.some((directory) => path.startsWith(directory))) {
      continue;
    }
    rust = true;
  }
  return { rust };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const full = process.env.GITHUB_EVENT_NAME !== 'pull_request';
  const base = process.env.BASE_SHA;
  if (!full && !/^[a-f0-9]{40}$/.test(base ?? '')) throw new Error('Missing pull-request base SHA');
  const paths = full ? [] : execFileSync('git', ['diff', '--no-renames', '--name-only', '-z', base, 'HEAD'], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }).split('\0').filter(Boolean);
  const selected = selectChecks(paths, full);
  for (const [name, enabled] of Object.entries(selected)) {
    appendFileSync(process.env.GITHUB_OUTPUT, `${name}=${enabled}\n`);
  }
  console.log(JSON.stringify({ paths, selected }, null, 2));
}
