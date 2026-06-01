import React from 'react';
import { CommandCardsView, SurfaceCard } from '../CommandCardsView';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';

// Arg-free, read-only Scientia commands (delegate to `vox db` handlers).
const SCIENTIA_CARDS: SurfaceCard[] = [
  {
    key: 'retrieval',
    title: 'Retrieval Status',
    description: 'Research ingest + retrieval readiness',
    path: ['scientia', 'retrieval-status'],
  },
  {
    key: 'discovery',
    title: 'Publication Discovery Queue',
    description: 'Candidate publications awaiting routing',
    path: ['scientia', 'publication-discovery-scan'],
  },
  {
    key: 'capability',
    title: 'Capability Map',
    description: 'Registered research capabilities',
    path: ['scientia', 'capability-list'],
  },
];

export function ScientiaDashboard({ pushToast }: SurfaceDecoratorProps) {
  return (
    <CommandCardsView
      title="Vox Scientia"
      subtitle="Research &amp; publication pipeline"
      cards={SCIENTIA_CARDS}
      pushToast={pushToast}
    />
  );
}
