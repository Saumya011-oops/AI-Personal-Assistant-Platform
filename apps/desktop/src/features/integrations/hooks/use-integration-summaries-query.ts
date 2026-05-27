import { useQuery } from '@tanstack/react-query';

import { invokeCommand } from '@/lib/api/invoke-command';

export function useIntegrationSummariesQuery() {
  return useQuery({
    queryKey: ['integrations'],
    queryFn: () => invokeCommand('list_integrations', {}),
  });
}
