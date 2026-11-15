// Nextest setup script: give every test process of this run a fresh workflow
// store, so no test can read or upgrade the developer's own saved work. Tests
// that need their own store still set MAGISTR_WORKFLOW_DATA_DIR explicitly.
//
// Node.js is already required to develop Magistr and has the same command on
// every platform, so one script serves them all.
import { appendFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const environment = process.env.NEXTEST_ENV;
if (!environment) {
  console.error('NEXTEST_ENV is not set; run this through cargo nextest.');
  process.exit(1);
}
const store = mkdtempSync(join(tmpdir(), 'magistr-test-store.'));
appendFileSync(environment, `MAGISTR_WORKFLOW_DATA_DIR=${store}\n`);
