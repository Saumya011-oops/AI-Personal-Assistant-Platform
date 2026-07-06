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

export const metadataDateRangeSchema = z.object({
  from: z.string().nullable().optional(),
  to: z.string().nullable().optional(),
});

export const metadataFiltersSchema = z.object({
  source: z.array(z.string()).nullable().optional(),
  author: z.array(z.string()).nullable().optional(),
  tags: z.array(z.string()).nullable().optional(),
  category: z.array(z.string()).nullable().optional(),
  dateRange: metadataDateRangeSchema.nullable().optional(),
});

export const retrievalStrategySchema = z.enum([
  'DENSE',
  'SPARSE',
  'HYBRID',
  'FACETED',
  'CONTEXTUAL',
  'RECURSIVE',
]);

export const queryComplexitySchema = z.enum(['simple', 'complex']);

export const queryAnalysisSchema = z.object({
  intent: z.string(),
  entities: z.array(z.string()),
  metadataFilters: metadataFiltersSchema,
  temporal: z.boolean(),
  complexity: queryComplexitySchema,
  strategy: retrievalStrategySchema,
});

export const retrievedChunkSchema = z.object({
  chunkId: z.string(),
  documentId: z.string(),
  source: z.string(),
  documentTitle: z.string(),
  content: z.string(),
  score: z.number(),
  ordinal: z.number().int(),
  pathOrUrl: z.string().nullable(),
  tags: z.array(z.string()),
  author: z.string().nullable(),
  category: z.string().nullable(),
  createdAt: z.string().nullable(),
  modifiedAt: z.string().nullable(),
  metadata: z.record(z.string(), z.unknown()),
});

export const retrievalResponseSchema = z.object({
  query: z.string(),
  strategyUsed: retrievalStrategySchema,
  analysis: queryAnalysisSchema,
  results: z.array(retrievedChunkSchema),
  totalResults: z.number().int().nonnegative(),
});

export const citationSchema = z.object({
  source: z.string(),
  documentId: z.string(),
  chunkId: z.string(),
  score: z.number(),
  sourceDocument: z.string(),                    // always populated by backend
  sourceType: z.string().optional(),
  retrievalScore: z.number().optional().nullable(),
  rerankScore: z.number(),                       // always populated by backend
  section: z.string().optional().nullable(),
  evidence: z.string().optional().nullable(),
  evidenceLevel: z.string().optional().nullable(),
  documentTitle: z.string(),
  evidenceSnippet: z.string().nullable().optional(),
  sourceConnector: z.string(),
});

export const assistantResponseSchema = z.object({
  answer: z.string(),
  citations: z.array(citationSchema),
  diagnostics: z.any().optional().nullable(),
  conversationId: z.string().optional().nullable(),
  memories: z.array(z.any()).optional().nullable(),
});

export type QueryAnalysis = z.infer<typeof queryAnalysisSchema>;
export type RetrievedChunk = z.infer<typeof retrievedChunkSchema>;
export type RetrievalResponse = z.infer<typeof retrievalResponseSchema>;
export type Citation = z.infer<typeof citationSchema>;
export type AssistantResponse = z.infer<typeof assistantResponseSchema>;
