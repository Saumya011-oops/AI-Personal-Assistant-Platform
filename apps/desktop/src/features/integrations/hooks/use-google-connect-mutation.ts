import { useMutation, useQueryClient } from '@tanstack/react-query';

import { invokeCommand } from '@/lib/api/invoke-command';

export function useGoogleConnectMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => invokeCommand('connect_google', {}),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['integrations'] }),
        queryClient.invalidateQueries({ queryKey: ['google-auth-status'] }),
      ]);
    },
  });
}
