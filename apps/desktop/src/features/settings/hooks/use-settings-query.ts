import { useQuery } from '@tanstack/react-query';

import { invokeCommand } from '@/lib/api/invoke-command';

export function useSettingsQuery() {
  return useQuery({
    queryKey: ['settings'],
    queryFn: () => invokeCommand('get_settings', {}),
  });
}
