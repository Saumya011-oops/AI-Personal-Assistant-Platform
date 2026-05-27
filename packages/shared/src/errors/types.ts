import { z } from 'zod';

export const appErrorSchema = z.object({
  code: z.string(),
  message: z.string(),
  details: z.record(z.string(), z.unknown()).optional(),
});

export const commandEnvelopeSchema = <T extends z.ZodTypeAny>(payload: T) =>
  z.object({
    success: z.boolean(),
    data: payload.nullable(),
    error: appErrorSchema.nullable(),
  });

export type AppError = z.infer<typeof appErrorSchema>;
