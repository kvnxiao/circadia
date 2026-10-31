import assert from 'node:assert/strict';
import test from 'node:test';
import { selectChecks } from './check-scope.mjs';

const none = { application: false, solar: false, reactor: false, settings: false, python: false, git: false };
const all = { application: true, solar: true, reactor: true, settings: true, python: true, git: true };

test('main, merge queues, and manual runs select all checks', () => {
  assert.deepEqual(selectChecks([], true), all);
});
test('documentation-only and empty diffs select no expensive checks', () => {
  assert.deepEqual(selectChecks(['README.md', 'docs/specs/interface.md']), none);
  assert.deepEqual(selectChecks([]), none);
});
test('application logic does not select unrelated probes', () => {
  assert.deepEqual(selectChecks(['src/schedule.rs']), { ...none, application: true, git: true });
});
test('composition code and shared assets also select settings', () => {
  for (const path of ['src/ui/composition/bindings.rs', 'assets/phase/day.svg']) {
    assert.deepEqual(selectChecks([path]), { ...none, application: true, settings: true, git: true });
  }
});
test('settings and Reactor dependencies select their consumers', () => {
  assert.deepEqual(selectChecks(['tools/windows-reactor/settings-design/src/main.rs']), { ...none, settings: true, git: true });
  assert.deepEqual(selectChecks(['tools/windows-reactor/Cargo.lock']), { ...none, reactor: true, settings: true, git: true });
});
test('solar timing selects Rust and Python coverage', () => {
  assert.deepEqual(selectChecks(['tools/solar-timing/measure.py']), { ...none, solar: true, python: true });
});
test('Python configuration selects Python checks', () => {
  assert.deepEqual(selectChecks(['tools/pyproject.toml']), { ...none, python: true });
});
test('unknown files and shared configuration conservatively select everything', () => {
  for (const path of ['Cargo.toml', 'Cargo.lock', '.github/workflows/ci.yml', '.github/workflows/preview.yml', 'new-package/main.rs']) {
    assert.deepEqual(selectChecks([path]), all);
  }
});
test('deleted paths and both sides of renames select all affected groups', () => {
  assert.deepEqual(selectChecks(['src/old.rs', 'tools/solar-calculation/src/new.rs']), { ...none, application: true, solar: true, git: true });
});
