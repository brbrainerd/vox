import { useEffect, useMemo, useState } from 'react';
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';
import { SETTINGS_INDEX } from '../components/surfaces/Settings/settingsIndex';
import { voxTransport } from '../transport';
import type { DocEntryLike } from '../components/layout/paletteSources';
import {
  buildFederatedIndex,
  searchFederatedIndex,
  type ActionManifestRow,
  type CommandCatalogRow,
  type FederatedIndexEntry,
  type FederatedIndexSources,
  type PolicySourceRow,
  type SearchFederatedIndexOptions,
} from '../lib/federatedSearchIndex';

export interface UseFederatedSearchIndexResult {
  entries: FederatedIndexEntry[];
  search: (query: string, options?: SearchFederatedIndexOptions) => FederatedIndexEntry[];
}

const POLICY_POLL_MS = 60_000;

/**
 * Memoized federated OmniSearch index from registry + settings + policies + docs + skills.
 * Policies refresh every 60s per contracts/gui/omnisearch-index.v1.yaml.
 */
export function useFederatedSearchIndex(skills: FederatedIndexSources['skills'] = []): UseFederatedSearchIndexResult {
  const [docs, setDocs] = useState<DocEntryLike[]>([]);
  const [policies, setPolicies] = useState<PolicySourceRow[]>([]);
  const [commands, setCommands] = useState<CommandCatalogRow[]>([]);
  const [actions, setActions] = useState<ActionManifestRow[]>([]);

  useEffect(() => {
    let cancelled = false;
    voxTransport
      .voxDocsIndex()
      .then(index => {
        // A misbehaving/incomplete backend mock (or a genuinely absent
        // command) can resolve with null/undefined; buildFederatedIndex
        // iterates sources.docs eagerly and unconditionally, so a nullish
        // value here crashes the whole app shell, not just search.
        if (!cancelled) setDocs(Array.isArray(index) ? index : []);
      })
      .catch(() => {
        if (!cancelled) setDocs([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    const loadPolicies = () => {
      voxTransport
        .listPolicies()
        .then(rows => {
          if (!cancelled) setPolicies(rows);
        })
        .catch(() => {
          if (!cancelled) setPolicies([]);
        });
    };

    loadPolicies();
    const intervalId = window.setInterval(loadPolicies, POLICY_POLL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    Promise.all([voxTransport.getCatalog(), voxTransport.getActionManifest()])
      .then(([catalog, manifest]) => {
        if (cancelled) return;
        setCommands(
          (catalog.entries ?? []).map(entry => ({
            command: entry.command,
            about: entry.about,
            path: entry.path,
          })),
        );
        setActions(
          (manifest.actions ?? []).map(entry => ({
            action_id: entry.id,
            title: entry.title,
            description: entry.description,
          })),
        );
      })
      .catch(() => {
        if (!cancelled) {
          setCommands([]);
          setActions([]);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const sources = useMemo<FederatedIndexSources>(
    () => ({
      surfaces: SURFACE_REGISTRY,
      settings: SETTINGS_INDEX,
      policies,
      docs,
      skills,
      commands,
      actions,
    }),
    [docs, policies, skills, commands, actions],
  );

  const entries = useMemo(() => buildFederatedIndex(sources), [sources]);

  const search = useMemo(
    () => (query: string, options?: SearchFederatedIndexOptions) =>
      searchFederatedIndex(entries, query, options),
    [entries],
  );

  return { entries, search };
}
