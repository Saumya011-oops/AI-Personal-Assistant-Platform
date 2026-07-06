import { z } from 'zod';

import {
  assistantResponseSchema,
  normalizedDocumentSchema,
  qdrantSearchResultSchema,
  retrievalResponseSchema,
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

export const assistantQueryInputSchema = z.object({
  query: z.string(),
  conversationId: z.string().nullable().optional(),
});

export const saveCredentialInputSchema = z.object({
  provider: z.string(),
  token: z.string(),
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
  save_credential: {
    input: saveCredentialInputSchema,
    output: z.unknown(),
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
  retrieve_documents: {
    input: assistantQueryInputSchema,
    output: retrievalResponseSchema,
  },
  ask_assistant: {
    input: assistantQueryInputSchema,
    output: assistantResponseSchema,
  },
  clear_all_documents: {
    input: emptyPayloadSchema,
    output: z.unknown(),
  },
  logout_and_reset: {
    input: emptyPayloadSchema,
    output: z.unknown(),
  },
  create_chat: {
    input: z.object({ title: z.string() }),
    output: z.string(),
  },
  list_chats: {
    input: emptyPayloadSchema,
    output: z.array(z.any()),
  },
  delete_chat: {
    input: z.object({ id: z.string() }),
    output: z.unknown(),
  },
  rename_chat: {
    input: z.object({ id: z.string(), title: z.string() }),
    output: z.unknown(),
  },
  search_chats: {
    input: z.object({ query: z.string() }),
    output: z.array(z.any()),
  },
  load_chat_messages: {
    input: z.object({ conversationId: z.string() }),
    output: z.array(z.any()),
  },
  get_conversation_summary: {
    input: z.object({ conversationId: z.string() }),
    output: z.string().nullable(),
  },
  list_memories: {
    input: emptyPayloadSchema,
    output: z.array(z.any()),
  },
  delete_memory: {
    input: z.object({ id: z.string() }),
    output: z.unknown(),
  },
  update_memory: {
    input: z.object({ id: z.string(), content: z.string(), importance: z.number() }),
    output: z.unknown(),
  },
  clear_all_memories: {
    input: emptyPayloadSchema,
    output: z.unknown(),
  },
  export_memories: {
    input: emptyPayloadSchema,
    output: z.string(),
  },
  import_memories: {
    input: z.object({ jsonStr: z.string() }),
    output: z.unknown(),
  },
  reset_assistant_data: {
    input: emptyPayloadSchema,
    output: z.unknown(),
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
