import { execFileSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export function selectChecks(paths, full = false) {
  let application = full;
  for (const path of paths) {
    if (path.startsWith('docs/') || path.startsWith('assets/screenshots/') || ['README.md', 'DEVELOPMENT.md', 'AGENTS.md', 'LICENSE'].includes(path)) {
      continue;
    }
    application = true;
  }
  return { application };
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
