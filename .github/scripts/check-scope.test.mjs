import assert from 'node:assert/strict';
import test from 'node:test';
import { selectChecks } from './check-scope.mjs';

const none = { application: false };
const all = { application: true };

test('main, merge queues, and manual runs select all checks', () => {
  assert.deepEqual(selectChecks([], true), all);
});
test('documentation-only and empty diffs select no expensive checks', () => {
  assert.deepEqual(selectChecks(['README.md', 'docs/specs/interface.md', 'assets/screenshots/schedule.png']), none);
  assert.deepEqual(selectChecks([]), none);
});
test('workspace crates select application checks', () => {
  for (const path of ['circadia-app/src/schedule.rs', 'circadia-app/assets/phase/day.svg', 'circadia-harness/src/main.rs', 'circadia-bindings/api.txt']) {
    assert.deepEqual(selectChecks([path]), all);
  }
});
test('unknown files and shared configuration conservatively select everything', () => {
  for (const path of ['Cargo.toml', 'Cargo.lock', '.github/workflows/ci.yml', '.github/workflows/preview.yml', 'new-package/main.rs']) {
    assert.deepEqual(selectChecks([path]), all);
  }
});
test('deleted paths and both sides of renames select affected checks', () => {
  assert.deepEqual(selectChecks(['docs/old.md', 'circadia-app/src/new.rs']), all);
});
