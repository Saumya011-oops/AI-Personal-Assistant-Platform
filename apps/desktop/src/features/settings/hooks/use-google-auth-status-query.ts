import { useQuery } from '@tanstack/react-query';

import { invokeCommand } from '@/lib/api/invoke-command';

export function useGoogleAuthStatusQuery() {
  return useQuery({
    queryKey: ['google-auth-status'],
    queryFn: () => invokeCommand('get_google_auth_status', {}),
  });
}
