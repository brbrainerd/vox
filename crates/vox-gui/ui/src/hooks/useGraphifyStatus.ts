// VG-1 G9: DEPRECATED — use useVoxGraphStatus instead.
// One-release back-compat re-export so any lingering importer (or vs1's
// forward-compat alias path) keeps working until the next release.
export {
  useVoxGraphStatus as useGraphifyStatus,
  useVoxGraphStatus as useVoxSearchStatus,
  VOX_GRAPH_STATUS_QUERY_KEY as GRAPHIFY_STATUS_QUERY_KEY,
} from './useVoxGraphStatus';
