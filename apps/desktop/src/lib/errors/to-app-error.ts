import type { AppError } from '@assistant/shared';

export function toAppError(
  code: string,
  message: string,
  details?: Record<string, unknown>,
): AppError {
  return {
    code,
    message,
    details,
  };
}
