import { useMutation, useQueryClient } from '@tanstack/react-query';

import { invokeCommand } from '@/lib/api/invoke-command';

export function useObsidianScanMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => invokeCommand('scan_obsidian_vault', {}),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['integrations'] }),
        queryClient.invalidateQueries({ queryKey: ['documents'] }),
      ]);
    },
  });
}
