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
      toSnakeCaseObject(parsedInput) as InvokeArgs,
    );
  } catch (error) {
    throw toAppError('COMMAND_FAILED', 'Failed to execute desktop command', {
      command,
      cause: error,
    });
  }

  const parsed = commandResultSchema(schema.output).parse(rawResult);
  if (!parsed.success || parsed.data === null || parsed.data === undefined) {
    throw parsed.error ?? toAppError('UNKNOWN', 'Command returned no data');
  }

  return parsed.data;
}

function toSnakeCaseObject(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(toSnakeCaseObject);
  }

  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, nestedValue]) => [
        key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`),
        toSnakeCaseObject(nestedValue),
      ]),
    );
  }

  return value;
}
