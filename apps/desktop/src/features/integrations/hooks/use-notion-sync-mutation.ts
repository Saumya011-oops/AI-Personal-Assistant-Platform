import { useMutation, useQueryClient } from '@tanstack/react-query';

import { invokeCommand } from '@/lib/api/invoke-command';

export function useNotionSyncMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => invokeCommand('sync_notion_documents', {}),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['integrations'] }),
        queryClient.invalidateQueries({ queryKey: ['documents'] }),
      ]);
    },
  });
}
