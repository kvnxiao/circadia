import assert from 'node:assert/strict';
import test from 'node:test';
import { selectChecks } from './check-scope.mjs';

const none = { rust: false };
const all = { rust: true };

test('main, merge queues, and manual runs select all checks', () => {
  assert.deepEqual(selectChecks([], true), all);
});
test('files that no build or test reads select no Rust checks', () => {
  assert.deepEqual(
    selectChecks([
      'README.md',
      'CONTRIBUTING.md',
      'LICENSE',
      'docs/specs/interface.md',
      'assets/screenshots/schedule.png',
      '.agents/skills/rust-rules/SKILL.md',
      '.github/ISSUE_TEMPLATE/bug_report.md',
      'packaging/circadia.iss',
      'circadia-harness/README.md',
      'circadia-bindings/README.md',
    ]),
    none,
  );
  assert.deepEqual(selectChecks([]), none);
});
test('workspace crates select Rust checks', () => {
  for (const path of [
    'circadia-app/src/schedule.rs',
    'circadia-app/assets/phase/day.svg',
    'circadia-harness/src/main.rs',
    'circadia-bindings/api.txt',
    'circadia-winsafe/src/message.rs',
  ]) {
    assert.deepEqual(selectChecks([path]), all);
  }
});
test('the circadia-winsafe README selects Rust checks because its inventory test reads it', () => {
  assert.deepEqual(selectChecks(['circadia-winsafe/README.md']), all);
});
test('unknown files and shared configuration conservatively select everything', () => {
  for (const path of ['Cargo.toml', 'Cargo.lock', 'justfile', '.github/workflows/ci.yml', '.github/scripts/check-scope.mjs', 'new-crate/README.md', 'new-package/main.rs']) {
    assert.deepEqual(selectChecks([path]), all);
  }
});
test('deleted paths and both sides of renames select affected checks', () => {
  assert.deepEqual(selectChecks(['docs/old.md', 'circadia-app/src/new.rs']), all);
});
