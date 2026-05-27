import { useQuery } from '@tanstack/react-query';

import { invokeCommand } from '@/lib/api/invoke-command';

export function useAppStatusQuery() {
  return useQuery({
    queryKey: ['app-status'],
    queryFn: () => invokeCommand('get_app_status', {}),
  });
}
