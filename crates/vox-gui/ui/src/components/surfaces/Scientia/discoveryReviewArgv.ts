/**
 * Pure argv builders for the DiscoveryReview surface. Kept separate from the
 * React component so the exact CLI flag wiring is unit-testable without a DOM.
 *
 * Both commands route through the shared `execute_command` __argv bridge — the
 * flag names mirror the clap definitions in
 * `vox-cli-core/src/scientia.rs` (`publication-claim-review` /
 * `publication-nanopub-build`).
 */

export type ReviewDecision = 'approve' | 'reject' | 'defer';

/** clap `value_enum` accepts the lowercased variant names. */
export const REVIEW_DECISIONS: ReviewDecision[] = ['approve', 'reject', 'defer'];

/**
 * argv for `vox scientia publication-claim-review`. `reason` is optional and
 * only appended when non-empty (the CLI flag is `Option<String>`).
 */
export function buildClaimReviewArgv(args: {
  publicationId: string;
  claimId: number;
  decision: ReviewDecision;
  reason?: string;
}): string[] {
  const argv = [
    '--publication-id',
    args.publicationId,
    '--claim-id',
    String(args.claimId),
    '--decision',
    args.decision,
  ];
  const reason = args.reason?.trim();
  if (reason) {
    argv.push('--reason', reason);
  }
  return argv;
}

/**
 * argv for `vox scientia publication-nanopub-build`. `orcid` is optional and
 * only appended when non-empty (the CLI flag is `Option<String>`).
 */
export function buildNanopubBuildArgv(args: {
  publicationId: string;
  claimId: number;
  orcid?: string;
}): string[] {
  const argv = ['--publication-id', args.publicationId, '--claim-id', String(args.claimId)];
  const orcid = args.orcid?.trim();
  if (orcid) {
    argv.push('--orcid', orcid);
  }
  return argv;
}

/**
 * Extract a Trusty URI from `publication-nanopub-build` stdout. The command
 * prints the resulting URI (`http://purl.org/np/RA...` / `RA...`); we surface
 * the first such token, else the trimmed stdout.
 */
export function extractTrustyUri(stdout: string): string {
  const match = stdout.match(/(?:https?:\/\/\S*\/)?RA[A-Za-z0-9_-]{20,}/);
  return match ? match[0] : stdout.trim();
}
