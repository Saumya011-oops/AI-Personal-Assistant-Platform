import {
  commandResultSchema,
  tauriCommandSchemas,
  type TauriCommandName,
} from '@assistant/shared';
import { invoke, type InvokeArgs } from '@tauri-apps/api/core';
import type { z } from 'zod';

import { toAppError } from '@/lib/errors/to-app-error';

type CommandInput<T extends TauriCommandName> = Parameters<
  typeof tauriCommandSchemas[T]['input']['parse']
>[0];

type CommandOutput<T extends TauriCommandName> = z.infer<
  (typeof tauriCommandSchemas)[T]['output']
>;

export async function invokeCommand<T extends TauriCommandName>(
  command: T,
  payload: CommandInput<T>,
): Promise<CommandOutput<T>> {
  const schema = tauriCommandSchemas[command];
  const parsedInput = schema.input.parse(payload);

  let rawResult: unknown;
  try {
    rawResult = await invoke(
      command,
      parsedInput as InvokeArgs,
    );
  } catch (error) {
    throw toAppError('COMMAND_FAILED', 'Failed to execute desktop command', {
      command,
      cause: error,
    });
  }

  const parsed = commandResultSchema(schema.output).parse(rawResult);
  if (!parsed.success) {
    throw parsed.error ?? toAppError('UNKNOWN', 'Command failed');
  }

  return parsed.data;
}

