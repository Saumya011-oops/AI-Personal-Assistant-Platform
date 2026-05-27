import { z } from 'zod';

export const appSettingsSchema = z.object({
  obsidianVaultPath: z.string().nullable(),
  preferredTheme: z.enum(['dark', 'system']).default('dark'),
  commandPaletteEnabled: z.boolean().default(true),
  telemetryEnabled: z.boolean().default(true),
});

export const updateSettingsInputSchema = appSettingsSchema.partial();

export type AppSettings = z.infer<typeof appSettingsSchema>;
export type UpdateSettingsInput = z.infer<typeof updateSettingsInputSchema>;
