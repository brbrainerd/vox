import { rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export default function globalSetup() {
  if (process.env.VOX_REVIEW_CAPTURE !== '1') return;
  const out = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'review-bundle', 'latest');
  rmSync(out, { recursive: true, force: true });
}
