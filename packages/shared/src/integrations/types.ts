import { z } from 'zod';

export const integrationKeySchema = z.enum([
  'notion',
  'obsidian',
  'google',
  'gmail',
  'google_calendar',
  'apple_calendar',
]);

export const integrationStatusSchema = z.enum([
  'not_connected',
  'connected',
  'syncing',
  'error',
]);

export const integrationSummarySchema = z.object({
  key: z.string(),
  label: z.string(),
  status: z.string(),
  lastSyncedAt: z.string().nullable(),
  detail: z.string().nullable(),
});

export const syncRunSchema = z.object({
  id: z.string(),
  integrationKey: z.string(),
  status: z.string(),
  startedAt: z.string(),
  finishedAt: z.string().nullable(),
  documentsDiscovered: z.number().int().nonnegative(),
  documentsUpserted: z.number().int().nonnegative(),
  errorMessage: z.string().nullable(),
});

export type IntegrationKey = z.infer<typeof integrationKeySchema>;
export type IntegrationSummary = z.infer<typeof integrationSummarySchema>;
export type SyncRun = z.infer<typeof syncRunSchema>;
