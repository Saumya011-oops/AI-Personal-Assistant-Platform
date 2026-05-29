import { z } from 'zod';

export const sourceKindSchema = z.enum([
  'notion',
  'obsidian',
  'gmail',
  'google_calendar',
  'apple_calendar',
]);

export const documentMetadataSchema = z.record(z.string(), z.unknown()).default({});

export const normalizedDocumentSchema = z.object({
  id: z.string(),
  sourceKind: z.string(),
  sourceExternalId: z.string(),
  title: z.string(),
  contentMarkdown: z.string(),
  contentPlaintext: z.string(),
  pathOrUrl: z.string().nullable(),
  tags: z.array(z.string()).default([]),
  createdAt: z.string().nullable(),
  updatedAt: z.string().nullable(),
  checksum: z.string(),
  metadata: documentMetadataSchema,
});

export const chunkRecordSchema = z.object({
  id: z.string(),
  documentId: z.string(),
  ordinal: z.number().int().nonnegative(),
  content: z.string(),
  tokenCount: z.number().int().nonnegative(),
  embeddingStatus: z.enum(['pending', 'completed', 'failed']),
  chunkLevel: z.enum(['standard', 'parent', 'child']).default('standard'),
  parentChunkId: z.string().nullable(),
});

export type SourceKind = z.infer<typeof sourceKindSchema>;
export type NormalizedDocument = z.infer<typeof normalizedDocumentSchema>;
export type ChunkRecord = z.infer<typeof chunkRecordSchema>;

export const qdrantSearchResultSchema = z.object({
  id: z.string(),
  score: z.number(),
  payload: z.record(z.string(), z.unknown()),
});

export type QdrantSearchResult = z.infer<typeof qdrantSearchResultSchema>;

// ─────────────────────────────────────────────────────────────────────────────
// Week 4 — Retrieval Layer Types
// ─────────────────────────────────────────────────────────────────────────────

export const retrievalStrategySchema = z.enum([
  'dense',
  'sparse',
  'hybrid',
  'faceted',
  'contextual',
  'recursive',
]);
export type RetrievalStrategy = z.infer<typeof retrievalStrategySchema>;

export const retrievalFiltersSchema = z.object({
  sourceKind: z.string().nullable().optional(),
  tags: z.array(z.string()).nullable().optional(),
  dateAfter: z.string().nullable().optional(),
  dateBefore: z.string().nullable().optional(),
});
export type RetrievalFilters = z.infer<typeof retrievalFiltersSchema>;

export const retrievalRequestSchema = z.object({
  query: z.string(),
  strategy: retrievalStrategySchema,
  limit: z.number().int().positive().nullable().optional(),
  filters: retrievalFiltersSchema.nullable().optional(),
  contextWindow: z.number().int().nonnegative().nullable().optional(),
});
export type RetrievalRequest = z.infer<typeof retrievalRequestSchema>;

export const contextChunkSchema = z.object({
  ordinal: z.number().int(),
  content: z.string(),
  isPrimary: z.boolean(),
});
export type ContextChunk = z.infer<typeof contextChunkSchema>;

export const retrievalResultSchema = z.object({
  chunkId: z.string(),
  documentId: z.string(),
  documentTitle: z.string(),
  sourceKind: z.string(),
  content: z.string(),
  score: z.number(),
  rank: z.number().int().positive(),
  strategy: z.string(),
  contextChunks: z.array(contextChunkSchema).default([]),
  parentContent: z.string().nullable(),
  pathOrUrl: z.string().nullable(),
  tags: z.array(z.string()).default([]),
});
export type RetrievalResult = z.infer<typeof retrievalResultSchema>;

export const retrievalResponseSchema = z.object({
  results: z.array(retrievalResultSchema),
  strategyUsed: z.string(),
  totalResults: z.number().int().nonnegative(),
  query: z.string(),
  latencyMs: z.number().int().nonnegative(),
});
export type RetrievalResponse = z.infer<typeof retrievalResponseSchema>;

export const allStrategiesResultSchema = z.object({
  query: z.string(),
  dense: retrievalResponseSchema,
  sparse: retrievalResponseSchema,
  hybrid: retrievalResponseSchema,
  faceted: retrievalResponseSchema,
  contextual: retrievalResponseSchema,
  recursive: retrievalResponseSchema,
  totalLatencyMs: z.number().int().nonnegative(),
});
export type AllStrategiesResult = z.infer<typeof allStrategiesResultSchema>;
