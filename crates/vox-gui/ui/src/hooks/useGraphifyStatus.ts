import { useQuery } from '@tanstack/react-query';
import { getGraphifyStatus } from '../transport';
import type { GraphifyStatusDto } from '../types/tauri';

export const GRAPHIFY_STATUS_QUERY_KEY = ['graphify', 'status'];

export function useGraphifyStatus() {
  return useQuery<GraphifyStatusDto, Error>({
    queryKey: GRAPHIFY_STATUS_QUERY_KEY,
    queryFn: getGraphifyStatus,
    staleTime: 30_000,
    refetchInterval: 60_000,
  });
}
