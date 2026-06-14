import { useQuery, useMutation, type UseQueryResult, type UseMutationResult, type QueryKey } from '@tanstack/react-query';

export function useVoxQuery<T>(
  queryKey: QueryKey,
  fetcher: () => Promise<T>,
  options?: { enabled?: boolean; staleTime?: number }
): UseQueryResult<T, Error> {
  return useQuery<T, Error>({
    queryKey,
    queryFn: fetcher,
    ...options,
  });
}

export function useVoxMutation<TData = void, TVariables = void>(
  mutator: (variables: TVariables) => Promise<TData>,
  options?: { onSuccess?: (data: TData) => void; onError?: (err: Error) => void }
): UseMutationResult<TData, Error, TVariables> {
  return useMutation<TData, Error, TVariables>({
    // Wrap to ensure only the variables argument is forwarded, not the TanStack
    // Query v5 context object that is appended as a second argument internally.
    mutationFn: (variables: TVariables) => mutator(variables),
    ...options,
  });
}
