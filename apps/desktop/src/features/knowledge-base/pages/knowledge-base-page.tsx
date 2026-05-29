import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  Brain,
  Search,
  Layers,
  Zap,
  Blend,
  Filter,
  BookOpen,
  RefreshCw,
  ChevronDown,
  ChevronRight,
  Clock,
  FileText,
  Tag,
  ExternalLink,
  BarChart2,
  SlidersHorizontal,
  GitBranch,
  Sparkles,
  X,
  AlertCircle,
} from 'lucide-react';

import type { RetrievalStrategy, RetrievalResult, AllStrategiesResult } from '@assistant/shared';
import { useRetrievalQuery, useAllStrategiesQuery } from '../hooks/use-retrieval-query';
import { invokeCommand } from '@/lib/api/invoke-command';

// ─────────────────────────────────────────────────────────────────────────────
// Strategy config
// ─────────────────────────────────────────────────────────────────────────────

interface StrategyMeta {
  id: RetrievalStrategy;
  label: string;
  shortLabel: string;
  description: string;
  placeholder: string;
  icon: React.ElementType;
  accent: string;
  badgeClass: string;
}

const STRATEGIES: StrategyMeta[] = [
  {
    id: 'dense',
    label: 'Dense Retrieval',
    shortLabel: 'Dense',
    description: 'Semantic vector search using Ollama embeddings + Qdrant cosine similarity. Best for conceptual queries.',
    placeholder: 'Ask a conceptual question…',
    icon: Brain,
    accent: 'from-violet-500 to-purple-600',
    badgeClass: 'bg-violet-500/10 text-violet-400 border-violet-500/20',
  },
  {
    id: 'sparse',
    label: 'Sparse / BM25',
    shortLabel: 'Sparse',
    description: 'SQLite FTS5 BM25 full-text keyword ranking. Best for exact terms, product names, error codes.',
    placeholder: 'Enter exact keywords or error codes…',
    icon: Layers,
    accent: 'from-amber-500 to-orange-600',
    badgeClass: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
  },
  {
    id: 'hybrid',
    label: 'Hybrid (RRF)',
    shortLabel: 'Hybrid',
    description: 'Reciprocal Rank Fusion combines dense + sparse results. Best general-purpose strategy.',
    placeholder: 'Any query — works best here…',
    icon: Blend,
    accent: 'from-emerald-500 to-teal-600',
    badgeClass: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
  },
  {
    id: 'faceted',
    label: 'Faceted Filtering',
    shortLabel: 'Faceted',
    description: 'Dense search with Qdrant payload filters applied. Scope results to a source, tag, or date range.',
    placeholder: 'Query within a filtered scope…',
    icon: Filter,
    accent: 'from-sky-500 to-blue-600',
    badgeClass: 'bg-sky-500/10 text-sky-400 border-sky-500/20',
  },
  {
    id: 'contextual',
    label: 'Contextual',
    shortLabel: 'Contextual',
    description: 'Returns the matching chunk plus its surrounding sibling chunks for richer passage context.',
    placeholder: 'Ask for context-rich passages…',
    icon: BookOpen,
    accent: 'from-pink-500 to-rose-600',
    badgeClass: 'bg-pink-500/10 text-pink-400 border-pink-500/20',
  },
  {
    id: 'recursive',
    label: 'Recursive',
    shortLabel: 'Recursive',
    description: 'Searches fine-grained child chunks, then loads the parent summary for broader context.',
    placeholder: 'Deep dive with summary context…',
    icon: GitBranch,
    accent: 'from-indigo-500 to-violet-600',
    badgeClass: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/20',
  },
];

// ─────────────────────────────────────────────────────────────────────────────
// Main Page
// ─────────────────────────────────────────────────────────────────────────────

export function KnowledgeBasePage() {
  const [query, setQuery] = useState('');
  const [activeStrategy, setActiveStrategy] = useState<RetrievalStrategy>('hybrid');
  const [compareMode, setCompareMode] = useState(false);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [sourceFilter, setSourceFilter] = useState('');
  const [limit, setLimit] = useState(8);
  const [contextWindow, setContextWindow] = useState(2);
  const [rebuildLoading, setRebuildLoading] = useState(false);
  const [rebuildMessage, setRebuildMessage] = useState('');

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { data, isLoading, error, search, clear } = useRetrievalQuery();
  const { data: compareData, isLoading: compareLoading, compare, clear: compareClear } = useAllStrategiesQuery();

  const strategyMeta = useMemo(
    (): StrategyMeta => STRATEGIES.find((s) => s.id === activeStrategy) ?? STRATEGIES[0]!,
    [activeStrategy],
  );

  // Debounced search trigger
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!query.trim()) {
      clear();
      compareClear();
      return;
    }

    debounceRef.current = setTimeout(() => {
      if (compareMode) {
        compare(query, limit);
      } else {
        search({
          query,
          strategy: activeStrategy,
          limit,
          contextWindow: activeStrategy === 'contextual' ? contextWindow : undefined,
          filters: sourceFilter
            ? { sourceKind: sourceFilter, tags: null, dateAfter: null, dateBefore: null }
            : null,
        });
      }
    }, 300);

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query, activeStrategy, compareMode, limit, contextWindow, sourceFilter]);

  function handleStrategyChange(id: RetrievalStrategy) {
    setActiveStrategy(id);
    compareClear();
    clear();
  }

  async function handleRebuildIndex() {
    setRebuildLoading(true);
    setRebuildMessage('');
    try {
      const count = await invokeCommand('rebuild_recursive_index', {});
      setRebuildMessage(`✅ Rebuilt recursive index for ${count} documents`);
    } catch (e) {
      setRebuildMessage(`❌ Rebuild failed: ${String(e)}`);
    } finally {
      setRebuildLoading(false);
    }
  }

  const results = compareMode ? null : data?.results ?? [];
  const isSearching = compareMode ? compareLoading : isLoading;

  return (
    <div className="flex h-[calc(100vh-8.5rem)] max-w-[1800px] mx-auto overflow-hidden rounded-2xl border border-outline-variant/20 bg-surface-container-lowest/60 backdrop-blur-sm animate-slide-up">

      {/* ── Left: Strategy & Controls ──────────────────── */}
      <aside className="flex w-[280px] shrink-0 flex-col border-r border-surface-container-highest overflow-hidden">

        {/* Header */}
        <div className="p-5 border-b border-surface-container-highest">
          <div className="flex items-center gap-2 mb-1">
            <Sparkles size={16} className="text-primary-glass" />
            <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline">
              Week 4 Retrieval
            </p>
          </div>
          <h2 className="text-lg font-bold text-on-surface">Knowledge Base</h2>
          <p className="text-[11px] text-on-surface-variant mt-1">6 retrieval strategies</p>
        </div>

        {/* Strategy Selector */}
        <div className="flex-1 overflow-y-auto custom-scrollbar p-3 space-y-1">
          <p className="font-mono text-[9px] font-bold uppercase tracking-widest text-outline px-2 py-1">
            Strategy
          </p>
          {STRATEGIES.map((s) => {
            const Icon = s.icon;
            const isActive = s.id === activeStrategy && !compareMode;
            return (
              <button
                key={s.id}
                type="button"
                id={`strategy-${s.id}`}
                onClick={() => { setCompareMode(false); handleStrategyChange(s.id); }}
                className={`w-full flex items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-all ${
                  isActive
                    ? 'bg-primary-glass/10 border border-primary-glass/25 text-primary-glass'
                    : 'border border-transparent text-on-surface-variant hover:bg-surface-container-high/40 hover:text-on-surface'
                }`}
              >
                <div className={`h-7 w-7 shrink-0 rounded-lg flex items-center justify-center bg-gradient-to-br ${s.accent} opacity-80`}>
                  <Icon size={14} className="text-white" />
                </div>
                <div className="min-w-0">
                  <p className="font-semibold text-[12px] truncate">{s.label}</p>
                </div>
                {isActive && <ChevronRight size={12} className="ml-auto shrink-0" />}
              </button>
            );
          })}

          {/* Compare Mode */}
          <div className="pt-3">
            <p className="font-mono text-[9px] font-bold uppercase tracking-widest text-outline px-2 py-1">
              Compare
            </p>
            <button
              type="button"
              id="strategy-compare"
              onClick={() => setCompareMode(!compareMode)}
              className={`w-full flex items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-all ${
                compareMode
                  ? 'bg-tertiary/10 border border-tertiary/25 text-tertiary'
                  : 'border border-transparent text-on-surface-variant hover:bg-surface-container-high/40 hover:text-on-surface'
              }`}
            >
              <div className="h-7 w-7 shrink-0 rounded-lg flex items-center justify-center bg-gradient-to-br from-slate-500 to-zinc-600 opacity-80">
                <BarChart2 size={14} className="text-white" />
              </div>
              <div className="min-w-0">
                <p className="font-semibold text-[12px]">All 6 Strategies</p>
                <p className="text-[10px] text-outline">Comparison Mode</p>
              </div>
            </button>
          </div>
        </div>

        {/* Options & Rebuild */}
        <div className="p-3 border-t border-surface-container-highest space-y-3">
          {/* Filters toggle */}
          <button
            type="button"
            onClick={() => setFiltersOpen(!filtersOpen)}
            className="w-full flex items-center gap-2 rounded-xl px-3 py-2 text-[12px] font-medium text-on-surface-variant hover:text-on-surface border border-outline-variant/20 hover:border-outline-variant/40 transition-all"
          >
            <SlidersHorizontal size={13} />
            Filters & Options
            <ChevronDown size={12} className={`ml-auto transition-transform ${filtersOpen ? 'rotate-180' : ''}`} />
          </button>

          {filtersOpen && (
            <div className="space-y-2 rounded-xl border border-outline-variant/15 bg-surface-container-high/30 p-3 animate-slide-up">
              <label className="block">
                <span className="font-mono text-[9px] font-bold uppercase tracking-widest text-outline">
                  Source
                </span>
                <select
                  className="mt-1 w-full rounded-lg border border-outline-variant/30 bg-surface-container-high/50 py-1.5 px-2 text-[12px] text-on-surface focus:outline-none focus:border-primary-glass/40"
                  value={sourceFilter}
                  onChange={(e) => setSourceFilter(e.target.value)}
                  id="filter-source"
                >
                  <option value="">All Sources</option>
                  <option value="notion">Notion</option>
                  <option value="obsidian">Obsidian</option>
                </select>
              </label>

              <label className="block">
                <span className="font-mono text-[9px] font-bold uppercase tracking-widest text-outline">
                  Results limit: {limit}
                </span>
                <input
                  type="range"
                  min={3}
                  max={20}
                  value={limit}
                  onChange={(e) => setLimit(Number(e.target.value))}
                  className="mt-1 w-full accent-primary-glass"
                  id="filter-limit"
                />
              </label>

              {activeStrategy === 'contextual' && !compareMode && (
                <label className="block">
                  <span className="font-mono text-[9px] font-bold uppercase tracking-widest text-outline">
                    Context window: ±{contextWindow} chunks
                  </span>
                  <input
                    type="range"
                    min={1}
                    max={5}
                    value={contextWindow}
                    onChange={(e) => setContextWindow(Number(e.target.value))}
                    className="mt-1 w-full accent-primary-glass"
                    id="filter-context-window"
                  />
                </label>
              )}
            </div>
          )}

          {/* Rebuild recursive index */}
          <button
            type="button"
            id="btn-rebuild-index"
            onClick={handleRebuildIndex}
            disabled={rebuildLoading}
            className="w-full flex items-center gap-2 justify-center rounded-xl px-3 py-2 text-[11px] font-semibold border border-outline-variant/20 text-outline hover:text-on-surface hover:border-outline-variant/40 transition-all disabled:opacity-50"
          >
            <RefreshCw size={12} className={rebuildLoading ? 'animate-spin' : ''} />
            {rebuildLoading ? 'Rebuilding…' : 'Rebuild Recursive Index'}
          </button>
          {rebuildMessage && (
            <p className="text-[10px] text-on-surface-variant text-center">{rebuildMessage}</p>
          )}
        </div>
      </aside>

      {/* ── Main: Query + Results ─────────────────────────── */}
      <div className="flex flex-1 flex-col min-w-0 overflow-hidden">

        {/* Search bar */}
        <div className="p-5 border-b border-surface-container-highest">
          {/* Strategy description */}
          {!compareMode && (
            <div className="flex items-start gap-3 mb-4">
              <div className={`h-8 w-8 shrink-0 rounded-xl flex items-center justify-center bg-gradient-to-br ${strategyMeta.accent}`}>
                <strategyMeta.icon size={16} className="text-white" />
              </div>
              <div>
                <p className="font-semibold text-[13px] text-on-surface">{strategyMeta.label}</p>
                <p className="text-[11px] text-on-surface-variant mt-0.5">{strategyMeta.description}</p>
              </div>
            </div>
          )}
          {compareMode && (
            <div className="flex items-center gap-3 mb-4">
              <div className="h-8 w-8 rounded-xl flex items-center justify-center bg-gradient-to-br from-slate-500 to-zinc-600">
                <BarChart2 size={16} className="text-white" />
              </div>
              <div>
                <p className="font-semibold text-[13px] text-on-surface">Comparison Mode</p>
                <p className="text-[11px] text-on-surface-variant">Running all 6 strategies in parallel</p>
              </div>
            </div>
          )}

          {/* Query input */}
          <div className="relative">
            <Search size={15} className="absolute left-3.5 top-1/2 -translate-y-1/2 text-outline pointer-events-none" />
            <input
              id="kb-search-input"
              className="w-full rounded-xl border border-outline-variant/30 bg-surface-container-high/50 py-3 pl-10 pr-10 text-[14px] text-on-surface placeholder:text-outline focus:outline-none focus:border-primary-glass/40 transition-colors"
              placeholder={compareMode ? 'Enter query to compare all 6 strategies…' : strategyMeta.placeholder}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              autoFocus
            />
            {query && (
              <button
                type="button"
                onClick={() => { setQuery(''); clear(); compareClear(); }}
                className="absolute right-3.5 top-1/2 -translate-y-1/2 text-outline hover:text-on-surface transition-colors"
              >
                <X size={14} />
              </button>
            )}
          </div>

          {/* Status bar */}
          {(data || compareData) && query && (
            <div className="flex items-center gap-4 mt-3">
              {!compareMode && data && (
                <>
                  <span className="font-mono text-[10px] text-outline">
                    {data.totalResults} results
                  </span>
                  <span className="font-mono text-[10px] text-outline flex items-center gap-1">
                    <Clock size={9} /> {data.latencyMs}ms
                  </span>
                  <span className={`font-mono text-[9px] px-2 py-0.5 rounded-full border ${strategyMeta.badgeClass}`}>
                    {data.strategyUsed}
                  </span>
                </>
              )}
              {compareMode && compareData && (
                <span className="font-mono text-[10px] text-outline flex items-center gap-1">
                  <Clock size={9} /> {compareData.totalLatencyMs}ms total
                </span>
              )}
            </div>
          )}
        </div>

        {/* Results area */}
        <div className="flex-1 overflow-y-auto custom-scrollbar">
          {isSearching && (
            <div className="flex flex-col items-center justify-center h-48 gap-3">
              <div className="h-8 w-8 rounded-full border-2 border-primary-glass/30 border-t-primary-glass animate-spin" />
              <p className="text-[12px] text-outline">
                {compareMode ? 'Running all 6 strategies…' : `Searching with ${strategyMeta.label}…`}
              </p>
            </div>
          )}

          {error && (
            <div className="m-6 rounded-xl border border-red-500/20 bg-red-500/5 p-4 flex items-start gap-3">
              <AlertCircle size={16} className="text-red-400 shrink-0 mt-0.5" />
              <div>
                <p className="font-semibold text-[12px] text-red-400">Retrieval Error</p>
                <p className="text-[11px] text-on-surface-variant mt-1">{error}</p>
              </div>
            </div>
          )}

          {/* No query */}
          {!query && !isSearching && (
            <div className="flex flex-col items-center justify-center h-full gap-4 text-center px-8">
              <div className="h-16 w-16 rounded-2xl bg-surface-container-highest border border-outline-variant/20 flex items-center justify-center">
                <Brain size={28} className="text-primary-glass" />
              </div>
              <div>
                <p className="text-[15px] font-semibold text-on-surface">
                  {compareMode ? 'Compare all 6 retrieval strategies' : `Try ${strategyMeta.label}`}
                </p>
                <p className="text-[12px] text-on-surface-variant mt-1 max-w-sm">
                  {compareMode
                    ? 'Type a query to see how each strategy ranks results side-by-side'
                    : strategyMeta.description}
                </p>
              </div>
            </div>
          )}

          {/* Single strategy results */}
          {!compareMode && !isSearching && results && results.length > 0 && (
            <div className="p-5 space-y-3">
              {results.map((result) => (
                <ResultCard key={result.chunkId} result={result} strategyMeta={strategyMeta} query={query} />
              ))}
            </div>
          )}

          {/* No results */}
          {!compareMode && !isSearching && data && results && results.length === 0 && query && !error && (
            <div className="flex flex-col items-center justify-center h-48 gap-3 text-center px-8">
              <Search size={24} className="text-outline" />
              <p className="text-[13px] font-semibold text-on-surface">No results found</p>
              <p className="text-[11px] text-on-surface-variant">
                Try a different query or switch to Hybrid strategy
              </p>
            </div>
          )}

          {/* Compare mode grid */}
          {compareMode && !compareLoading && compareData && query && (
            <CompareGrid data={compareData} query={query} />
          )}
        </div>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Result Card
// ─────────────────────────────────────────────────────────────────────────────

function ResultCard({
  result,
  strategyMeta,
  query,
}: {
  result: RetrievalResult;
  strategyMeta: StrategyMeta;
  query: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const hasContext = result.contextChunks.length > 1;
  const hasParent = !!result.parentContent;

  return (
    <div className="rounded-xl border border-outline-variant/15 bg-surface-container-high/20 hover:border-outline-variant/30 transition-all overflow-hidden">
      {/* Card header */}
      <div className="p-4">
        <div className="flex items-start justify-between gap-3 mb-3">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-mono text-[9px] font-bold px-2 py-0.5 rounded-full border bg-surface-container-highest text-outline border-outline-variant/30">
              #{result.rank}
            </span>
            <span className={`font-mono text-[9px] font-bold px-2 py-0.5 rounded-full border ${
              result.sourceKind === 'notion' ? 'badge-notion' : 'badge-obsidian'
            }`}>
              {result.sourceKind}
            </span>
            {hasContext && (
              <span className="font-mono text-[9px] px-2 py-0.5 rounded-full border bg-pink-500/5 text-pink-400 border-pink-500/20">
                +{result.contextChunks.length - 1} context chunks
              </span>
            )}
            {hasParent && (
              <span className="font-mono text-[9px] px-2 py-0.5 rounded-full border bg-indigo-500/5 text-indigo-400 border-indigo-500/20">
                parent loaded
              </span>
            )}
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <ScoreBar score={result.score} accent={strategyMeta.accent} />
            <span className="font-mono text-[10px] font-bold text-on-surface">
              {(result.score * 100).toFixed(1)}%
            </span>
          </div>
        </div>

        <p className="font-semibold text-[13px] text-on-surface mb-1 flex items-center gap-1.5">
          <FileText size={12} className="text-outline shrink-0" />
          {result.documentTitle}
        </p>

        {result.tags.length > 0 && (
          <div className="flex items-center gap-1 flex-wrap mb-2">
            <Tag size={9} className="text-outline" />
            {result.tags.slice(0, 4).map((tag) => (
              <span key={tag} className="font-mono text-[9px] text-outline bg-surface-container-highest rounded px-1.5 py-0.5">
                {tag}
              </span>
            ))}
          </div>
        )}

        {/* Chunk content */}
        <p className="text-[12px] leading-relaxed text-on-surface-variant">
          {highlightText(result.content.slice(0, expanded ? 800 : 220), query)}
          {!expanded && result.content.length > 220 && (
            <button
              type="button"
              onClick={() => setExpanded(true)}
              className="ml-1 text-primary-glass hover:underline text-[11px]"
            >
              show more
            </button>
          )}
        </p>

        {result.pathOrUrl && (
          <a
            href={result.pathOrUrl}
            target="_blank"
            rel="noreferrer"
            className="mt-2 inline-flex items-center gap-1 text-[11px] text-primary-glass hover:underline"
          >
            Source <ExternalLink size={10} />
          </a>
        )}
      </div>

      {/* Parent content (recursive) */}
      {hasParent && expanded && (
        <div className="border-t border-outline-variant/10 bg-indigo-500/3 p-4">
          <p className="font-mono text-[9px] font-bold uppercase tracking-widest text-indigo-400 mb-2">
            Parent Summary
          </p>
          <p className="text-[11px] leading-relaxed text-on-surface-variant italic">
            {result.parentContent!.slice(0, 400)}…
          </p>
        </div>
      )}

      {/* Context window (contextual) */}
      {hasContext && expanded && (
        <div className="border-t border-outline-variant/10 bg-pink-500/3 p-4">
          <p className="font-mono text-[9px] font-bold uppercase tracking-widest text-pink-400 mb-2">
            Context Window
          </p>
          <div className="space-y-2">
            {result.contextChunks.map((c) => (
              <div
                key={c.ordinal}
                className={`rounded-lg p-2.5 text-[11px] leading-relaxed ${
                  c.isPrimary
                    ? 'border border-pink-500/25 bg-pink-500/5 text-on-surface'
                    : 'text-on-surface-variant'
                }`}
              >
                <span className="font-mono text-[9px] text-outline mr-2">#{c.ordinal}</span>
                {c.content.slice(0, 200)}
              </div>
            ))}
          </div>
        </div>
      )}

      {(hasParent || hasContext) && (
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="w-full flex items-center justify-center gap-1 py-2 text-[10px] text-outline hover:text-on-surface transition-colors border-t border-outline-variant/10"
        >
          {expanded ? 'Collapse' : 'Expand context'}
          <ChevronDown size={10} className={`transition-transform ${expanded ? 'rotate-180' : ''}`} />
        </button>
      )}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Compare Grid
// ─────────────────────────────────────────────────────────────────────────────

function CompareGrid({ data, query }: { data: AllStrategiesResult; query: string }) {
  const strategies: Array<{ key: keyof AllStrategiesResult; meta: StrategyMeta }> = STRATEGIES.map(
    (s) => ({ key: s.id as keyof AllStrategiesResult, meta: s }),
  );

  return (
    <div className="p-5">
      <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
        {strategies.map(({ key, meta }) => {
          const response = data[key] as any;
          const Icon = meta.icon;
          return (
            <div
              key={key}
              className="rounded-xl border border-outline-variant/15 bg-surface-container-high/20 overflow-hidden"
            >
              {/* Strategy header */}
              <div className={`flex items-center gap-2 p-3 bg-gradient-to-r ${meta.accent} bg-opacity-5`}>
                <div className={`h-6 w-6 rounded-lg flex items-center justify-center bg-gradient-to-br ${meta.accent}`}>
                  <Icon size={12} className="text-white" />
                </div>
                <p className="font-semibold text-[12px] text-on-surface">{meta.label}</p>
                <div className="ml-auto flex items-center gap-2">
                  <span className="font-mono text-[9px] text-outline">{response.totalResults} hits</span>
                  <span className="font-mono text-[9px] text-outline">{response.latencyMs}ms</span>
                </div>
              </div>

              {/* Top 3 results */}
              <div className="p-3 space-y-2">
                {response.results.slice(0, 3).map((r: RetrievalResult) => (
                  <div
                    key={r.chunkId}
                    className="rounded-lg border border-outline-variant/10 bg-surface-container-highest/30 p-2.5"
                  >
                    <div className="flex items-center justify-between mb-1">
                      <span className="font-mono text-[9px] text-outline">#{r.rank}</span>
                      <span className="font-mono text-[9px] font-bold text-on-surface">
                        {(r.score * 100).toFixed(1)}%
                      </span>
                    </div>
                    <p className="font-semibold text-[11px] text-on-surface truncate">{r.documentTitle}</p>
                    <p className="text-[10px] text-on-surface-variant mt-0.5 line-clamp-2">
                      {r.content.slice(0, 100)}
                    </p>
                  </div>
                ))}
                {response.results.length === 0 && (
                  <p className="text-[11px] text-outline text-center py-4">No results</p>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Score Bar
// ─────────────────────────────────────────────────────────────────────────────

function ScoreBar({ score, accent }: { score: number; accent: string }) {
  return (
    <div className="w-16 h-1.5 rounded-full bg-surface-container-highest overflow-hidden">
      <div
        className={`h-full rounded-full bg-gradient-to-r ${accent} transition-all`}
        style={{ width: `${Math.max(2, score * 100)}%` }}
      />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Highlight helper
// ─────────────────────────────────────────────────────────────────────────────

function highlightText(text: string, query: string) {
  const normalizedQuery = query.trim();
  if (!normalizedQuery) return text;
  const parts = text.split(new RegExp(`(${escapeRegExp(normalizedQuery)})`, 'gi'));
  return parts.map((part, index) =>
    part.toLowerCase() === normalizedQuery.toLowerCase() ? (
      <mark key={`${part}-${index}`} className="rounded bg-primary-glass/15 px-0.5 text-on-surface">
        {part}
      </mark>
    ) : (
      part
    ),
  );
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
