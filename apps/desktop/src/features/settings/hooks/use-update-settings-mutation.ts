import type { UpdateSettingsInput } from '@assistant/shared';
import { useMutation, useQueryClient } from '@tanstack/react-query';

import { invokeCommand } from '@/lib/api/invoke-command';

export function useUpdateSettingsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: UpdateSettingsInput) =>
      invokeCommand('update_settings', payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ['settings'] });
    },
  });
}
