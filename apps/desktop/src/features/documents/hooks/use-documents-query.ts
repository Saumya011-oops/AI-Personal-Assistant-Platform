import { useQuery } from '@tanstack/react-query';

import { invokeCommand } from '@/lib/api/invoke-command';

export function useDocumentsQuery() {
  return useQuery({
    queryKey: ['documents'],
    queryFn: () => invokeCommand('list_documents', {}),
  });
}
