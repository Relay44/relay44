import { useQuery } from '@tanstack/react-query';
import { resolveBasename } from '@/lib/basenames';

export function useBasename(address: string | undefined) {
  const { data: basename, isLoading } = useQuery({
    queryKey: ['basename', address],
    queryFn: () => resolveBasename(address!),
    enabled: !!address,
    staleTime: 5 * 60 * 1000,
    gcTime: 10 * 60 * 1000,
  });

  return { basename: basename ?? null, isLoading };
}
