import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { BookOpen, Database, ExternalLink, FileText, Search, Sparkles, Trash2, X } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { EmptyState } from '@/components/states/empty-state';
import { LoadingState } from '@/components/states/loading-state';
import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';
import { invokeCommand } from '@/lib/api/invoke-command';

const sourceIcon = (kind: string) => {
  switch (kind) {
    case 'notion': return <BookOpen className="h-4 w-4 text-indigo-400" />;
    case 'obsidian': return <Database className="h-4 w-4 text-purple-400" />;
    default: return <FileText className="h-4 w-4 text-slate-400" />;
  }
};

const sourceBadgeColor = (kind: string) => {
  switch (kind) {
    case 'notion': return 'bg-indigo-500/10 text-indigo-300 border-indigo-500/30';
    case 'obsidian': return 'bg-purple-500/10 text-purple-300 border-purple-500/30';
    default: return 'bg-slate-500/10 text-slate-300 border-slate-500/30';
  }
};

export function DocumentsPage() {
  const query = useDocumentsQuery();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');
  const [sourceFilter, setSourceFilter] = useState<string>('all');
  const [semanticMode, setSemanticMode] = useState(false);

  const allDocs = query.data ?? [];

  // Get unique source kinds
  const sourceKinds = useMemo(() => {
    const kinds = new Set(allDocs.map(d => d.sourceKind));
    return Array.from(kinds);
  }, [allDocs]);

  // Semantic query hook (only active in semanticMode when search is entered)
  const semanticQuery = useQuery({
    queryKey: ['documents-semantic', search],
    queryFn: () => invokeCommand('search_documents_semantic', { query: search, limit: 15 }),
    enabled: semanticMode && search.trim().length > 0,
  });

  // Clear documents mutation
  const clearMutation = useMutation({
    mutationFn: () => invokeCommand('clear_all_documents', {}),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['documents'] });
      queryClient.invalidateQueries({ queryKey: ['integrations'] });
      queryClient.invalidateQueries({ queryKey: ['app-status'] });
      setSearch('');
    },
  });

  // Filter full-text documents locally for instant feedback
  const filteredDocs = useMemo(() => {
    let docs = allDocs;
    if (sourceFilter !== 'all') {
      docs = docs.filter(d => d.sourceKind === sourceFilter);
    }
    if (search.trim()) {
      const q = search.toLowerCase();
      docs = docs.filter(d =>
        d.title.toLowerCase().includes(q) ||
        d.contentPlaintext.toLowerCase().includes(q)
      );
    }
    return docs;
  }, [allDocs, search, sourceFilter]);

  // Filter semantic results locally by source if filter is active
  const filteredSemanticResults = useMemo(() => {
    let results = semanticQuery.data ?? [];
    if (sourceFilter !== 'all') {
      results = results.filter(r => r.payload.source === sourceFilter);
    }
    return results;
  }, [semanticQuery.data, sourceFilter]);

  if (query.isLoading) {
    return <LoadingState label="Loading documents from local store…" />;
  }

  if (!allDocs.length) {
    return (
      <EmptyState
        title="No documents indexed yet"
        description="Run Notion sync or Obsidian vault scan to populate your knowledge base."
        action={
          <Button variant="secondary" onClick={() => navigate('/integrations')}>
            Go to Integrations
          </Button>
        }
      />
    );
  }

  return (
    <div className="space-y-4">
      {/* Header + stats */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold">Documents</h2>
          <p className="mt-0.5 text-sm text-slate-400">
            {allDocs.length} total indexed documents
          </p>
        </div>
        <Button
          variant="secondary"
          size="sm"
          onClick={() => {
            if (confirm("Are you sure you want to clear all documents, chunks, and vector embeddings? This cannot be undone.")) {
              clearMutation.mutate();
            }
          }}
          disabled={clearMutation.isPending}
          className="flex items-center gap-2 bg-red-950/40 text-red-400 border border-red-500/25 hover:bg-red-900/30 hover:text-red-300 transition-colors"
        >
          <Trash2 className="h-4 w-4" />
          {clearMutation.isPending ? 'Clearing Index…' : 'Clear Index'}
        </Button>
      </div>

      {/* Search mode tabs */}
      <div className="flex items-center justify-between border-b border-border/40 pb-1">
        <div className="flex gap-4">
          <button
            onClick={() => setSemanticMode(false)}
            className={`border-b-2 pb-1.5 text-sm font-medium transition-colors ${
              !semanticMode
                ? 'border-indigo-500 text-indigo-400 font-semibold'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            Full-Text Search
          </button>
          <button
            onClick={() => setSemanticMode(true)}
            className={`flex items-center gap-1.5 border-b-2 pb-1.5 text-sm font-medium transition-colors ${
              semanticMode
                ? 'border-indigo-500 text-indigo-400 font-semibold'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Sparkles className="h-3.5 w-3.5" />
            AI Semantic Search (Local Qdrant)
          </button>
        </div>
      </div>

      {/* Search + filter bar */}
      <div className="flex flex-col gap-3 sm:flex-row">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
          <input
            className="w-full rounded-2xl border border-border/60 bg-white/5 py-2.5 pl-10 pr-10 text-sm outline-none focus:border-indigo-500/60 focus:ring-1 focus:ring-indigo-500/30"
            placeholder={
              semanticMode
                ? "Ask a question or type a concept (e.g. 'RAG specification' or 'Notion keys')…"
                : "Search by title or content…"
            }
            value={search}
            onChange={e => setSearch(e.target.value)}
          />
          {search && (
            <button
              className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-500 hover:text-slate-300"
              onClick={() => setSearch('')}
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>

        {/* Source filter pills */}
        <div className="flex gap-2">
          {['all', ...sourceKinds].map(kind => (
            <button
              key={kind}
              onClick={() => setSourceFilter(kind)}
              className={`rounded-xl border px-3 py-1.5 text-xs font-medium capitalize transition-colors ${
                sourceFilter === kind
                  ? 'border-indigo-500/60 bg-indigo-500/20 text-indigo-300'
                  : 'border-border/60 bg-white/5 text-slate-400 hover:border-border hover:text-slate-300'
              }`}
            >
              {kind}
            </button>
          ))}
        </div>
      </div>

      {/* Results panel */}
      {semanticMode ? (
        // --- AI SEMANTIC SEARCH RESULTS ---
        search.trim().length === 0 ? (
          <div className="flex flex-col items-center justify-center rounded-2xl border border-dashed border-border/60 py-16 text-center text-slate-500">
            <Sparkles className="mb-3 h-10 w-10 text-indigo-500/40" />
            <p className="max-w-md text-sm leading-relaxed">
              Enter a search query or a question above to perform a local semantic similarity search against the Qdrant vector database using nomic-embed-text!
            </p>
          </div>
        ) : semanticQuery.isLoading ? (
          <LoadingState label="Computing query embedding & querying Qdrant vector database…" />
        ) : semanticQuery.isError ? (
          <div className="rounded-2xl border border-red-500/30 bg-red-500/10 p-6 text-center text-red-400">
            Failed to run semantic search: {String(semanticQuery.error)}
          </div>
        ) : filteredSemanticResults.length === 0 ? (
          <div className="rounded-2xl border border-border/60 p-8 text-center text-slate-400">
            No semantic matches found for "{search}"
          </div>
        ) : (
          <div className="space-y-3">
            <p className="text-xs text-slate-500 px-1">
              Top semantic matches from Qdrant:
            </p>
            {filteredSemanticResults.map(result => {
              const payload = result.payload as Record<string, any>;
              const source = (payload.source as string) || 'unknown';
              const title = (payload.title as string) || 'Untitled Match';
              const content = (payload.content as string) || '';
              const pathOrUrl = (payload.path_or_url as string) || null;
              const matchPercentage = Math.round(result.score * 100);

              return (
                <Card key={result.id} className="relative overflow-hidden border-indigo-500/20 bg-indigo-500/[0.02]">
                  {/* Subtle top indicator bar */}
                  <div className="absolute top-0 left-0 right-0 h-[2px] bg-gradient-to-r from-indigo-500/40 via-purple-500/40 to-transparent" />
                  
                  <div className="flex items-start justify-between gap-4">
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        {sourceIcon(source)}
                        <span className={`rounded-full border px-2 py-0.5 text-xs font-medium ${sourceBadgeColor(source)}`}>
                          {source}
                        </span>
                        
                        {/* Similarity Score Badge */}
                        <span className="rounded-md bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 px-1.5 py-0.5 text-[10px] font-bold">
                          {matchPercentage}% Semantic Match
                        </span>
                      </div>
                      
                      <h3 className="mt-2 truncate text-base font-semibold text-slate-200">{title}</h3>
                      
                      {/* Highlighted text snippet */}
                      <div className="mt-2 rounded-xl bg-black/30 p-3 border border-border/40">
                        <p className="line-clamp-4 text-xs font-mono text-slate-300 leading-relaxed">
                          {content || 'No text content snippet available'}
                        </p>
                      </div>
                    </div>

                    {pathOrUrl && (
                      <a
                        href={pathOrUrl}
                        target="_blank"
                        rel="noreferrer"
                        className="mt-1 shrink-0 text-slate-500 hover:text-slate-300"
                        title="Open source"
                      >
                        <ExternalLink className="h-4 w-4" />
                      </a>
                    )}
                  </div>
                </Card>
              );
            })}
          </div>
        )
      ) : (
        // --- STANDARD FULL-TEXT SEARCH RESULTS ---
        filteredDocs.length === 0 ? (
          <div className="rounded-2xl border border-border/60 p-8 text-center text-slate-400">
            No documents match "{search}"
          </div>
        ) : (
          <div className="space-y-3">
            {filteredDocs.map(document => (
              <Card key={document.id}>
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      {sourceIcon(document.sourceKind)}
                      <span className={`rounded-full border px-2 py-0.5 text-xs font-medium ${sourceBadgeColor(document.sourceKind)}`}>
                        {document.sourceKind}
                      </span>
                      {document.tags.length > 0 && (
                        <span className="text-xs text-slate-500">{document.tags.length} tags</span>
                      )}
                    </div>
                    <h3 className="mt-2 truncate text-base font-semibold">{document.title}</h3>
                    <p className="mt-1 line-clamp-2 text-sm text-slate-400">
                      {document.contentPlaintext.slice(0, 200) || 'No content preview'}
                    </p>
                    {document.updatedAt && (
                      <p className="mt-1 text-xs text-slate-600">
                        Updated {new Date(document.updatedAt).toLocaleDateString()}
                      </p>
                    )}
                  </div>
                  {document.pathOrUrl && (
                    <a
                      href={document.pathOrUrl}
                      target="_blank"
                      rel="noreferrer"
                      className="mt-1 shrink-0 text-slate-500 hover:text-slate-300"
                      title="Open source"
                    >
                      <ExternalLink className="h-4 w-4" />
                    </a>
                  )}
                </div>
              </Card>
            ))}
          </div>
        )
      )}
    </div>
  );
}
