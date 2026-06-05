import { useVirtualizer } from '@tanstack/react-virtual';
import {
  startTransition,
  useEffect,
  type KeyboardEvent,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  Search,
  FileText,
  ExternalLink,
  LayoutList,
  BarChart2,
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

import { EmptyState } from '@/components/states/empty-state';
import { LoadingState } from '@/components/states/loading-state';
import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';

export function DocumentsPage() {
  const query = useDocumentsQuery();
  const [search, setSearch] = useState('');
  const [source, setSource] = useState<'all' | 'notion' | 'obsidian'>('all');
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'details' | 'analysis'>('details');
  const listRef = useRef<HTMLDivElement | null>(null);
  const previewRef = useRef<HTMLDivElement | null>(null);
  const previousDocumentIdRef = useRef<string | null>(null);
  const previewScrollPositionsRef = useRef<Record<string, number>>({});

  const documents = query.data;

  const filteredDocuments = useMemo(() => {
    const normalizedQuery = search.trim().toLowerCase();
    return (documents ?? []).filter((document) => {
      const matchesSource = source === 'all' || document.sourceKind === source;
      const matchesQuery =
        normalizedQuery.length === 0 ||
        document.title.toLowerCase().includes(normalizedQuery) ||
        document.contentPlaintext.toLowerCase().includes(normalizedQuery) ||
        document.tags.some((tag) => tag.toLowerCase().includes(normalizedQuery));
      return matchesSource && matchesQuery;
    });
  }, [documents, search, source]);

  useEffect(() => {
    if (!filteredDocuments.length) {
      setSelectedDocumentId(null);
      return;
    }
    const hasActiveDocument = filteredDocuments.some((d) => d.id === selectedDocumentId);
    if (!hasActiveDocument) {
      setSelectedDocumentId(filteredDocuments[0]?.id ?? null);
    }
  }, [filteredDocuments, selectedDocumentId]);

  const activeIndex = Math.max(
    filteredDocuments.findIndex((d) => d.id === selectedDocumentId),
    0,
  );
  const activeDocument = filteredDocuments[activeIndex] ?? filteredDocuments[0] ?? null;

  useLayoutEffect(() => {
    const previewElement = previewRef.current;
    const previousId = previousDocumentIdRef.current;
    if (previewElement && previousId) {
      previewScrollPositionsRef.current[previousId] = previewElement.scrollTop;
    }
    if (previewElement && activeDocument) {
      previewElement.scrollTop = previewScrollPositionsRef.current[activeDocument.id] ?? 0;
    }
    previousDocumentIdRef.current = activeDocument?.id ?? null;
  }, [activeDocument]);

  const rowVirtualizer = useVirtualizer({
    count: filteredDocuments.length,
    getScrollElement: () => listRef.current,
    estimateSize: () => 120,
    overscan: 8,
  });

  const retrievalSegments = useMemo(() => {
    if (!activeDocument) return [];
    return activeDocument.contentPlaintext
      .split(/\n{2,}/)
      .map((s) => s.trim())
      .filter(Boolean)
      .slice(0, 6);
  }, [activeDocument]);

  const relatedDocuments = useMemo(() => {
    if (!activeDocument) return [];
    return filteredDocuments
      .filter(
        (d) =>
          d.id !== activeDocument.id &&
          (d.sourceKind === activeDocument.sourceKind ||
            d.tags.some((tag) => activeDocument.tags.includes(tag))),
      )
      .slice(0, 4);
  }, [activeDocument, filteredDocuments]);

  function handleSelectDocument(id: string) {
    startTransition(() => setSelectedDocumentId(id));
  }

  function handleListKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (!filteredDocuments.length) return;
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const offset = event.key === 'ArrowDown' ? 1 : -1;
    const nextIndex = Math.min(Math.max(activeIndex + offset, 0), filteredDocuments.length - 1);
    const nextDocument = filteredDocuments[nextIndex];
    if (!nextDocument) return;
    handleSelectDocument(nextDocument.id);
    rowVirtualizer.scrollToIndex(nextIndex, { align: 'auto' });
  }

  if (query.isLoading) {
    return <LoadingState label="Loading indexed documents from the local store." />;
  }

  if (!documents?.length) {
    return (
      <EmptyState
        description="Run a Notion sync or scan your Obsidian vault to populate the knowledge explorer."
        title="Knowledge base is still empty"
      />
    );
  }

  if (!activeDocument) {
    return (
      <EmptyState
        description="Adjust the active filters to bring documents back into view."
        title="No document matches the current filter"
      />
    );
  }

  return (
    <div className="flex h-[calc(100vh-8.5rem)] max-w-[1700px] mx-auto overflow-hidden rounded-2xl border border-outline-variant/20 bg-surface-container-lowest/60 backdrop-blur-sm animate-slide-up">
      {/* ── Left: Document List ──────────────────────────── */}
      <div className="flex w-[360px] shrink-0 flex-col border-r border-surface-container-highest overflow-hidden">
        {/* List header */}
        <div className="p-5 border-b border-surface-container-highest space-y-3">
          <div className="flex items-center justify-between">
            <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline">
              Results
            </p>
            <span className="font-mono text-[11px] text-primary-glass">
              {documents.length} indexed
            </span>
          </div>
          <h3 className="text-xl font-bold text-on-surface">Documents</h3>

          {/* Search */}
          <div className="relative">
            <Search
              size={15}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-outline pointer-events-none"
            />
            <input
              className="w-full rounded-xl border border-outline-variant/30 bg-surface-container-high/50 py-2.5 pl-9 pr-3 text-[13px] text-on-surface placeholder:text-outline focus:outline-none focus:border-primary-glass/40 transition-colors"
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search titles, content, tags…"
              value={search}
              id="documents-search"
            />
          </div>

          {/* Source filter */}
          <div className="flex gap-2">
            {(['all', 'notion', 'obsidian'] as const).map((value) => (
              <button
                key={value}
                className={`rounded-lg border px-3 py-1 text-[11px] font-medium transition-all ${
                  source === value
                    ? 'border-primary-glass/30 bg-primary-glass/10 text-primary-glass'
                    : 'border-outline-variant/20 bg-surface-container-high/40 text-outline hover:text-on-surface'
                }`}
                onClick={() => setSource(value)}
                type="button"
                id={`filter-${value}`}
              >
                {value === 'all' ? 'All' : value.charAt(0).toUpperCase() + value.slice(1)}
              </button>
            ))}
          </div>
        </div>

        {/* Virtual document list */}
        <div
          ref={listRef}
          className="flex-1 overflow-auto custom-scrollbar px-3 py-3 outline-none"
          onKeyDown={handleListKeyDown}
          tabIndex={0}
        >
          <div
            className="relative w-full"
            style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
          >
            {rowVirtualizer.getVirtualItems().map((virtualRow) => {
              const document = filteredDocuments[virtualRow.index];
              if (!document) return null;
              const isActive = document.id === activeDocument.id;

              return (
                <div
                  key={document.id}
                  ref={rowVirtualizer.measureElement}
                  data-index={virtualRow.index}
                  className="absolute left-0 top-0 w-full px-1 py-1"
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                >
                  <button
                    className={`w-full rounded-xl p-4 text-left transition-all ${
                      isActive
                        ? 'border border-primary-glass/30 bg-primary-glass/8 shadow-sm'
                        : 'border border-transparent bg-surface-container-high/20 hover:bg-surface-container-high/40 hover:border-outline-variant/20'
                    }`}
                    onClick={() => handleSelectDocument(document.id)}
                    type="button"
                  >
                    <div className="flex items-center gap-2 mb-2">
                      <FileText
                        size={14}
                        className={isActive ? 'text-primary-glass' : 'text-outline'}
                      />
                      <span
                        className={`font-mono text-[9px] font-bold uppercase px-1.5 py-0.5 rounded ${
                          document.sourceKind === 'notion' ? 'badge-notion' : 'badge-obsidian'
                        }`}
                      >
                        {document.sourceKind}
                      </span>
                    </div>
                    <p className={`font-semibold text-[13px] mb-1 ${isActive ? 'text-primary-glass' : 'text-on-surface'}`}>
                      {highlightText(document.title, search)}
                    </p>
                    <p className="text-[12px] text-on-surface-variant line-clamp-2 leading-relaxed">
                      {highlightText(document.contentPlaintext.slice(0, 120), search)}
                    </p>
                    {isActive && (
                      <p className="mt-2 text-[10px] font-mono text-outline">CHECKSUM TRACKED</p>
                    )}
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* ── Right: Detail / Analysis panes ───────────────── */}
      <div className="flex flex-1 flex-col overflow-hidden min-w-0">
        {/* Tab bar */}
        <div className="flex items-center border-b border-surface-container-highest/40 bg-surface-container-lowest/30 sticky top-0 z-10">
          <nav className="flex relative">
            {([
              { key: 'details', Icon: LayoutList, label: 'Document Details' },
              { key: 'analysis', Icon: BarChart2, label: 'Context Analysis' },
            ] as const).map((tab) => {
              const isActive = activeTab === tab.key;
              return (
                <button
                  key={tab.key}
                  onClick={() => setActiveTab(tab.key)}
                  className={`relative flex items-center gap-2 px-6 py-4 text-[12px] font-semibold transition-colors ${
                    isActive ? 'text-primary-glass font-bold' : 'text-outline hover:text-on-surface'
                  }`}
                  type="button"
                  id={`tab-${tab.key}`}
                >
                  {isActive && (
                    <motion.div
                      layoutId="active-documents-tab"
                      className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary-glass"
                      transition={{ type: 'spring', stiffness: 380, damping: 30 }}
                    />
                  )}
                  <tab.Icon size={16} className="relative z-10" />
                  <span className="relative z-10">{tab.label}</span>
                </button>
              );
            })}
          </nav>
        </div>

        {/* Tab Content */}
        <div
          ref={previewRef}
          className="flex-1 overflow-y-auto custom-scrollbar bg-transparent"
        >
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={`${activeDocument.id}-${activeTab}`}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.15, ease: 'easeOut' }}
              className="h-full min-h-full"
            >
              {activeTab === 'details' ? (
                <div className="px-8 py-8 max-w-3xl">
                  {/* Badges + source link */}
                  <div className="flex items-center justify-between gap-3 flex-wrap mb-6">
                    <div className="flex items-center gap-2">
                      <span
                        className={`rounded-full px-3 py-1 text-[10px] font-bold uppercase border ${
                          activeDocument.sourceKind === 'notion' ? 'badge-notion' : 'badge-obsidian'
                        }`}
                      >
                        {activeDocument.sourceKind}
                      </span>
                      <span className="rounded-full border border-outline-variant/30 bg-surface-container px-3 py-1 text-[10px] font-semibold uppercase text-outline">
                        ID Verified
                      </span>
                    </div>
                    {activeDocument.pathOrUrl && (
                      <a
                        className="flex items-center gap-1 text-[13px] text-primary-glass hover:underline"
                        href={activeDocument.pathOrUrl}
                        rel="noreferrer"
                        target="_blank"
                      >
                        Source Origin
                        <ExternalLink size={14} />
                      </a>
                    )}
                  </div>

                  {/* Title */}
                  <h1 className="text-3xl font-bold text-on-surface tracking-tight leading-tight mb-6">
                    {activeDocument.title}
                  </h1>

                  {/* Meta grid */}
                  <div className="grid grid-cols-3 gap-4 py-5 border-y border-surface-container-highest/40 mb-6">
                    <div className="space-y-1">
                      <p className="font-mono text-[10px] uppercase tracking-widest text-outline">
                        Created
                      </p>
                      <p className="font-mono text-on-surface font-semibold text-[13px]">
                        {activeDocument.createdAt
                          ? new Date(activeDocument.createdAt).toLocaleDateString('en-US', {
                              month: 'short',
                              day: 'numeric',
                              year: 'numeric',
                            })
                          : 'Unknown'}
                      </p>
                      <p className="font-mono text-[10px] text-outline">
                        {activeDocument.createdAt
                          ? new Date(activeDocument.createdAt).toLocaleTimeString('en-US', {
                              hour: '2-digit',
                              minute: '2-digit',
                              timeZoneName: 'short',
                            })
                          : ''}
                      </p>
                    </div>
                    <div className="space-y-1">
                      <p className="font-mono text-[10px] uppercase tracking-widest text-outline">
                        Last Updated
                      </p>
                      <p className="font-mono text-on-surface font-semibold text-[13px]">
                        {activeDocument.updatedAt
                          ? new Date(activeDocument.updatedAt).toLocaleDateString('en-US', {
                              month: 'short',
                              day: 'numeric',
                              year: 'numeric',
                            })
                          : 'Unknown'}
                      </p>
                      <p className="font-mono text-[10px] text-outline">
                        {activeDocument.updatedAt
                          ? new Date(activeDocument.updatedAt).toLocaleTimeString('en-US', {
                              hour: '2-digit',
                              minute: '2-digit',
                              timeZoneName: 'short',
                            })
                          : ''}
                      </p>
                    </div>
                    <div className="space-y-1 overflow-hidden">
                      <p className="font-mono text-[10px] uppercase tracking-widest text-outline">
                        System ID
                      </p>
                      <p className="font-mono text-on-surface font-semibold text-[13px] truncate">
                        {activeDocument.sourceExternalId.slice(0, 20)}…
                      </p>
                      <p className="font-mono text-[10px] text-outline">SHA-256 Hash</p>
                    </div>
                  </div>

                  {/* Document preview label */}
                  <p className="font-mono text-[10px] uppercase tracking-widest text-outline text-center mb-6">
                    Document Preview
                  </p>

                  {/* Content */}
                  <div className="space-y-4">
                    {activeDocument.contentPlaintext
                      .split(/\n{2,}/)
                      .filter(Boolean)
                      .map((segment, index) => (
                        <p
                          key={`${activeDocument.id}-${index}`}
                          className="text-[15px] leading-relaxed text-on-surface-variant"
                        >
                          {highlightText(segment, search)}
                        </p>
                      ))}
                  </div>
                </div>
              ) : (
                <div className="p-8 grid grid-cols-1 lg:grid-cols-2 gap-8">
                  {/* Chunk Candidates */}
                  <div className="space-y-4">
                    <div className="flex items-center justify-between">
                      <h4 className="font-mono text-[10px] font-bold uppercase tracking-wider text-outline">
                        Knowledge Chunks
                      </h4>
                      <span className="font-mono text-[10px] rounded border border-outline-variant/30 bg-surface-container-highest px-2 py-0.5 text-outline">
                        {String(retrievalSegments.length).padStart(2, '0')}
                      </span>
                    </div>
                    <motion.div 
                      variants={{
                        hidden: { opacity: 0 },
                        show: { opacity: 1, transition: { staggerChildren: 0.05 } }
                      }}
                      initial="hidden"
                      animate="show"
                      className="space-y-3"
                    >
                      {retrievalSegments.map((segment, index) => (
                        <motion.div
                          key={`${activeDocument.id}-chunk-${index}`}
                          variants={{
                            hidden: { opacity: 0, y: 8 },
                            show: { opacity: 1, y: 0, transition: { type: 'spring', stiffness: 300, damping: 25 } }
                          }}
                          className="rounded-xl border border-tertiary/15 bg-tertiary/5 p-4"
                        >
                          <div className="flex items-center justify-between mb-2">
                            <span className="font-mono text-[9px] font-bold uppercase text-outline">
                              NODE_{String(index + 1).padStart(2, '0')}_A
                            </span>
                            <span className="font-mono text-[9px] text-primary-glass">
                              Chunk {index + 1}
                            </span>
                          </div>
                          <p className="text-[12px] leading-relaxed text-on-surface-variant italic">
                            "{segment.slice(0, 200)}…"
                          </p>
                        </motion.div>
                      ))}
                    </motion.div>
                  </div>

                  {/* Related Documents */}
                  <div className="space-y-4">
                    <h4 className="font-mono text-[10px] font-bold uppercase tracking-wider text-outline">
                      Related Documents
                    </h4>
                    {relatedDocuments.length === 0 ? (
                      <p className="text-[13px] text-on-surface-variant">
                        No closely related documents visible in the current filter set.
                      </p>
                    ) : (
                      <motion.div 
                        variants={{
                          hidden: { opacity: 0 },
                          show: { opacity: 1, transition: { staggerChildren: 0.05 } }
                        }}
                        initial="hidden"
                        animate="show"
                        className="space-y-3"
                      >
                        {relatedDocuments.map((doc) => (
                          <motion.button
                            key={doc.id}
                            variants={{
                              hidden: { opacity: 0, y: 8 },
                              show: { opacity: 1, y: 0, transition: { type: 'spring', stiffness: 300, damping: 25 } }
                            }}
                            whileHover={{ y: -2, borderColor: 'rgba(142, 213, 255, 0.25)', backgroundColor: 'rgba(14, 25, 47, 0.3)' }}
                            className="flex w-full items-center gap-4 rounded-xl border border-transparent p-4 transition-all group text-left"
                            onClick={() => handleSelectDocument(doc.id)}
                            type="button"
                          >
                            <div className="h-10 w-10 shrink-0 rounded-xl bg-surface-container-highest flex items-center justify-center border border-outline-variant/30 group-hover:border-primary-glass/30">
                              <FileText size={18} className="text-primary-glass" />
                            </div>
                            <div className="min-w-0 flex-1">
                              <p className="font-semibold text-on-surface text-[13px] group-hover:text-primary-glass transition-colors truncate">
                                {doc.title}
                              </p>
                              <p className="text-[11px] text-outline mt-0.5">{doc.sourceKind}</p>
                            </div>
                          </motion.button>
                        ))}
                      </motion.div>
                    )}
                  </div>
                </div>
              )}
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </div>
  );
}

// ── Helpers ──────────────────────────────────────────────────
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
