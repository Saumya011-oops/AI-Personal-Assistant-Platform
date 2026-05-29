import { z } from 'zod';

import {
  normalizedDocumentSchema,
  qdrantSearchResultSchema,
  retrievalRequestSchema,
  retrievalResponseSchema,
  allStrategiesResultSchema,
} from '../documents/types';
import { appErrorSchema } from '../errors/types';
import { integrationSummarySchema, syncRunSchema } from '../integrations/types';
import { appSettingsSchema, updateSettingsInputSchema } from '../settings/types';

export const emptyPayloadSchema = z.object({});

export const appStatusSchema = z.object({
  appVersion: z.string(),
  environment: z.string(),
  rustBackendAvailable: z.boolean(),
  databaseReady: z.boolean(),
});

export const selectVaultInputSchema = z.object({
  path: z.string(),
});

export const googleAuthStatusSchema = z.object({
  connected: z.boolean(),
  email: z.string().nullable(),
  expiresAt: z.string().datetime().nullable(),
});

export const oauthCallbackInputSchema = z.object({
  code: z.string(),
  state: z.string(),
});

export const syncNotionInputSchema = z.object({
  cursor: z.string().nullable().optional(),
});

export const listDocumentsInputSchema = z.object({
  sourceKind: z.string().nullable().optional(),
  query: z.string().nullable().optional(),
});

export const searchDocumentsSemanticInputSchema = z.object({
  query: z.string(),
  limit: z.number().int().nonnegative().nullable().optional(),
});

export const tauriCommandSchemas = {
  get_app_status: {
    input: emptyPayloadSchema,
    output: appStatusSchema,
  },
  list_integrations: {
    input: emptyPayloadSchema,
    output: z.array(integrationSummarySchema),
  },
  get_settings: {
    input: emptyPayloadSchema,
    output: appSettingsSchema,
  },
  update_settings: {
    input: updateSettingsInputSchema,
    output: appSettingsSchema,
  },
  select_obsidian_vault: {
    input: selectVaultInputSchema,
    output: appSettingsSchema,
  },
  scan_obsidian_vault: {
    input: emptyPayloadSchema,
    output: syncRunSchema,
  },
  connect_google: {
    input: emptyPayloadSchema,
    output: googleAuthStatusSchema,
  },
  oauth_callback: {
    input: oauthCallbackInputSchema,
    output: googleAuthStatusSchema,
  },
  get_google_auth_status: {
    input: emptyPayloadSchema,
    output: googleAuthStatusSchema,
  },
  sync_notion_documents: {
    input: syncNotionInputSchema,
    output: syncRunSchema,
  },
  list_documents: {
    input: listDocumentsInputSchema,
    output: z.array(normalizedDocumentSchema),
  },
  search_documents_semantic: {
    input: searchDocumentsSemanticInputSchema,
    output: z.array(qdrantSearchResultSchema),
  },
  clear_all_documents: {
    input: emptyPayloadSchema,
    output: z.unknown(),
  },
  // Week 4 — Retrieval Layer
  retrieve_documents: {
    input: retrievalRequestSchema,
    output: retrievalResponseSchema,
  },
  test_retrieval_strategies: {
    input: z.object({
      query: z.string(),
      limit: z.number().int().positive().nullable().optional(),
    }),
    output: allStrategiesResultSchema,
  },
  rebuild_recursive_index: {
    input: emptyPayloadSchema,
    output: z.number().int().nonnegative(),
  },
} as const;

export const commandResultSchema = <T extends z.ZodTypeAny>(schema: T) =>
  z.object({
    success: z.boolean(),
    data: schema.nullable(),
    error: appErrorSchema.nullable(),
  });

export type TauriCommandName = keyof typeof tauriCommandSchemas;
export type AppStatus = z.infer<typeof appStatusSchema>;
export type GoogleAuthStatus = z.infer<typeof googleAuthStatusSchema>;
