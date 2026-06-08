import { describe, expect, it } from 'vitest';
import {
  buildClaimReviewArgv,
  buildNanopubBuildArgv,
  extractTrustyUri,
  REVIEW_DECISIONS,
} from './discoveryReviewArgv';

describe('buildClaimReviewArgv', () => {
  it('emits the required flags in CLI order, stringifying claim id', () => {
    expect(
      buildClaimReviewArgv({ publicationId: 'pub-1', claimId: 42, decision: 'approve' })
    ).toEqual(['--publication-id', 'pub-1', '--claim-id', '42', '--decision', 'approve']);
  });

  it('appends --reason only when non-empty', () => {
    expect(
      buildClaimReviewArgv({ publicationId: 'p', claimId: 1, decision: 'reject', reason: 'unsupported' })
    ).toEqual(['--publication-id', 'p', '--claim-id', '1', '--decision', 'reject', '--reason', 'unsupported']);
    expect(
      buildClaimReviewArgv({ publicationId: 'p', claimId: 1, decision: 'defer', reason: '   ' })
    ).not.toContain('--reason');
  });

  it('only allows the three clap value_enum decisions', () => {
    expect(REVIEW_DECISIONS).toEqual(['approve', 'reject', 'defer']);
  });
});

describe('buildNanopubBuildArgv', () => {
  it('emits publication + claim id without orcid by default', () => {
    expect(buildNanopubBuildArgv({ publicationId: 'pub-1', claimId: 7 })).toEqual([
      '--publication-id',
      'pub-1',
      '--claim-id',
      '7',
    ]);
  });

  it('appends --orcid when provided', () => {
    expect(
      buildNanopubBuildArgv({ publicationId: 'p', claimId: 2, orcid: 'https://orcid.org/0000-0002-1825-0097' })
    ).toEqual([
      '--publication-id',
      'p',
      '--claim-id',
      '2',
      '--orcid',
      'https://orcid.org/0000-0002-1825-0097',
    ]);
  });

  it('skips blank orcid', () => {
    expect(buildNanopubBuildArgv({ publicationId: 'p', claimId: 2, orcid: '  ' })).not.toContain('--orcid');
  });
});

describe('extractTrustyUri', () => {
  it('pulls a full purl Trusty URI from noisy stdout', () => {
    const out = 'signed ok\nhttp://purl.org/np/RAaBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789\n';
    expect(extractTrustyUri(out)).toBe('http://purl.org/np/RAaBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789');
  });

  it('falls back to trimmed stdout when no URI is present', () => {
    expect(extractTrustyUri('  no uri here  ')).toBe('no uri here');
  });
});
