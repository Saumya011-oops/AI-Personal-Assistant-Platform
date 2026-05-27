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

