import { execFileSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export function selectChecks(paths, full = false) {
  const selected = { application: full, solar: full, reactor: full, settings: full, python: full };
  for (const path of paths) {
    if (path.startsWith('docs/') || ['README.md', 'DEVELOPMENT.md', 'AGENTS.md', 'LICENSE'].includes(path)) {
      continue;
    }
    if (path.startsWith('src/') || path.startsWith('assets/')) {
      selected.application = true;
      if (path.startsWith('src/ui/composition/') || path.startsWith('assets/')) selected.settings = true;
    } else if (path.startsWith('i18n/') || path.startsWith('tests/') || path === 'build.rs' || path.startsWith('tools/native') || path.startsWith('tools/composition-bindings')) {
      selected.application = true;
    } else if (path.startsWith('tools/windows-reactor/settings-design/')) {
      selected.settings = true;
    } else if (path.startsWith('tools/windows-reactor/')) {
      selected.reactor = true;
      selected.settings = true;
    } else if (path.startsWith('tools/solar-calculation/')) {
      selected.solar = true;
    } else if (path.startsWith('tools/solar-timing/')) {
      selected.solar = true;
      selected.python = true;
    } else if (path.startsWith('tools/windows-clock/') || ['tools/pyproject.toml', 'tools/uv.lock', 'tools/justfile'].includes(path)) {
      selected.python = true;
    } else {
      for (const name of Object.keys(selected)) selected[name] = true;
    }
    if (path.endsWith('.py')) selected.python = true;
  }
  return { ...selected, git: selected.application || selected.reactor || selected.settings };
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
