import React from 'react';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';

interface HarnessRedirectProps {
  onFocusComposer?: () => void;
}

/**
 * Legacy Quick Harness tab — composer parity (slash, model tier, diff) lives in Loquela.
 */
export function HarnessRedirect({ onFocusComposer }: HarnessRedirectProps) {
  return (
    <section className="space-y-4" aria-labelledby="harness-redirect-title">
      <EmptyState
        icon={<Icon.bolt className="size-8" />}
        title="Quick Harness lives in the composer"
        description="Submit tasks, pick models, run /plan · /verify · /diff, and review worktree diffs from the Loquela bar at the bottom of the console."
        action={
          onFocusComposer
            ? { label: 'Focus composer', onClick: onFocusComposer }
            : undefined
        }
      />
      <p id="harness-redirect-title" className="text-center text-[10px] text-zinc-600">
        Workspace → Quick Harness retained for deep links; execution surface is Loquela.
      </p>
    </section>
  );
}
