import { useVirtualizer } from '@tanstack/react-virtual';
import {
  ArrowUpRight,
  BookOpen,
  Database,
  FileText,
  Search,
  Tags,
} from 'lucide-react';
import {
  startTransition,
  useEffect,
  type KeyboardEvent,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';

import { EmptyState } from '@/components/states/empty-state';
import { LoadingState } from '@/components/states/loading-state';
import { Badge } from '@/components/ui/badge';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';
import { Separator } from '@/components/ui/separator';
import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';

const sourceMeta = {
  notion: {
    label: 'Notion',
    icon: BookOpen,
    badge: 'secondary' as const,
  },
  obsidian: {
    label: 'Obsidian',
    icon: Database,
    badge: 'success' as const,
  },
};

export function DocumentsPage() {
  const query = useDocumentsQuery();
  const [search, setSearch] = useState('');
  const [source, setSource] = useState<'all' | 'notion' | 'obsidian'>('all');
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(null);
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

    const hasActiveDocument = filteredDocuments.some(
      (document) => document.id === selectedDocumentId,
    );

    if (!hasActiveDocument) {
      setSelectedDocumentId(filteredDocuments[0]?.id ?? null);
    }
  }, [filteredDocuments, selectedDocumentId]);

  const activeIndex = Math.max(
    filteredDocuments.findIndex((document) => document.id === selectedDocumentId),
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
    estimateSize: () => 132,
    overscan: 8,
  });

  const retrievalSegments = useMemo(() => {
    if (!activeDocument) {
      return [];
    }

    return activeDocument.contentPlaintext
      .split(/\n{2,}/)
      .map((segment) => segment.trim())
      .filter(Boolean)
      .slice(0, 6);
  }, [activeDocument]);

  const relatedDocuments = useMemo(() => {
    if (!activeDocument) {
      return [];
    }

    return filteredDocuments
      .filter(
        (document) =>
          document.id !== activeDocument.id &&
          (document.sourceKind === activeDocument.sourceKind ||
            document.tags.some((tag) => activeDocument.tags.includes(tag))),
      )
      .slice(0, 4);
  }, [activeDocument, filteredDocuments]);

  function handleSelectDocument(id: string) {
    startTransition(() => {
      setSelectedDocumentId(id);
    });
  }

  function handleListKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (!filteredDocuments.length) {
      return;
    }

    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') {
      return;
    }

    event.preventDefault();

    const offset = event.key === 'ArrowDown' ? 1 : -1;
    const nextIndex = Math.min(
      Math.max(activeIndex + offset, 0),
      filteredDocuments.length - 1,
    );
    const nextDocument = filteredDocuments[nextIndex];

    if (!nextDocument) {
      return;
    }

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
    <div className="mx-auto flex h-[calc(100vh-6.25rem)] max-w-[1700px] min-h-[760px] flex-col gap-4">
      <Card className="p-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
              Knowledge base
            </p>
            <h2 className="mt-2 text-2xl font-semibold tracking-tight">
              Split-view document explorer for fast retrieval browsing
            </h2>
            <p className="mt-2 max-w-3xl text-sm leading-6 text-muted-foreground">
              Browse the corpus like a desktop workspace: filter quickly, switch documents
              instantly, and keep preview context visible while you inspect metadata.
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Badge variant="outline">{documents?.length ?? 0} indexed</Badge>
            <Badge variant="secondary">Keyboard navigation</Badge>
          </div>
        </div>

        <div className="mt-5 flex flex-wrap gap-3">
          <div className="relative min-w-[320px] flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              className="pl-10"
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search titles, content, or tags…"
              value={search}
            />
          </div>
          <div className="flex gap-2">
            {(['all', 'notion', 'obsidian'] as const).map((value) => (
              <button
                key={value}
                className={`rounded-2xl border px-3 py-2 text-sm transition ${
                  source === value
                    ? 'border-primary/30 bg-primary/10 text-primary'
                    : 'border-border bg-secondary/60 text-muted-foreground hover:text-foreground'
                }`}
                onClick={() => setSource(value)}
                type="button"
              >
                {value === 'all' ? 'All sources' : sourceMeta[value].label}
              </button>
            ))}
          </div>
        </div>
      </Card>

      <ResizablePanelGroup
        className="min-h-0 flex-1 rounded-[30px] border border-border bg-card/65"
        direction="horizontal"
      >
        <ResizablePanel defaultSize={29} minSize={24}>
          <div className="flex h-full flex-col">
            <div className="px-5 py-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Results
              </p>
              <h3 className="mt-2 text-lg font-semibold">{filteredDocuments.length} retrievable document{filteredDocuments.length === 1 ? '' : 's'}</h3>
              <p className="mt-1 text-xs text-muted-foreground">
                Use arrow keys to move through the list.
              </p>
            </div>
            <Separator />
            <div
              ref={listRef}
              className="min-h-0 flex-1 overflow-auto px-3 py-3 outline-none"
              onKeyDown={handleListKeyDown}
              tabIndex={0}
            >
              <div
                className="relative w-full"
                style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
              >
                {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                  const document = filteredDocuments[virtualRow.index];
                  const meta =
                    document
                      ? sourceMeta[document.sourceKind as keyof typeof sourceMeta] ?? {
                          label: document.sourceKind,
                          icon: FileText,
                          badge: 'outline' as const,
                        }
                      : {
                          label: 'Unknown',
                          icon: FileText,
                          badge: 'outline' as const,
                        };
                  const Icon = meta.icon;

                  if (!document) {
                    return null;
                  }
                  const isActive = document.id === activeDocument.id;

                  return (
                    <div
                      key={document.id}
                      className="absolute left-0 top-0 w-full px-1 py-1"
                      style={{ transform: `translateY(${virtualRow.start}px)` }}
                    >
                      <button
                        className={`w-full rounded-[24px] border px-4 py-4 text-left transition ${
                          isActive
                            ? 'border-primary/30 bg-primary/10'
                            : 'border-transparent bg-secondary/50 hover:border-border'
                        }`}
                        onClick={() => handleSelectDocument(document.id)}
                        type="button"
                      >
                        <div className="flex items-center gap-2">
                          <Icon className="h-4 w-4 text-primary" />
                          <Badge variant={meta.badge}>{meta.label}</Badge>
                          {document.tags[0] ? (
                            <Badge variant="outline">{document.tags[0]}</Badge>
                          ) : null}
                        </div>
                        <p className="mt-3 text-sm font-medium">
                          {highlightText(document.title, search)}
                        </p>
                        <p className="mt-2 line-clamp-3 text-sm leading-6 text-muted-foreground">
                          {highlightText(document.contentPlaintext.slice(0, 180), search)}
                        </p>
                      </button>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </ResizablePanel>

        <ResizableHandle withHandle />

        <ResizablePanel defaultSize={46} minSize={34}>
          <div className="flex h-full flex-col">
            <div className="flex items-center justify-between gap-4 px-6 py-4">
              <div className="min-w-0">
                <p className="truncate text-lg font-semibold">{activeDocument.title}</p>
                <p className="mt-1 text-sm text-muted-foreground">
                  Switch documents without losing explorer context
                </p>
              </div>
              {activeDocument.pathOrUrl ? (
                <a
                  className="inline-flex items-center gap-1 text-sm text-primary hover:underline"
                  href={activeDocument.pathOrUrl}
                  rel="noreferrer"
                  target="_blank"
                >
                  Open source
                  <ArrowUpRight className="h-4 w-4" />
                </a>
              ) : null}
            </div>
            <Separator />
            <div ref={previewRef} className="min-h-0 flex-1 overflow-auto px-6 py-5">
              <div className="space-y-6">
                <div className="flex flex-wrap gap-2">
                  <Badge variant="outline">{activeDocument.sourceKind}</Badge>
                  <Badge variant="secondary">Checksum tracked</Badge>
                  <Badge variant="secondary">
                    {activeDocument.tags.length} tag{activeDocument.tags.length === 1 ? '' : 's'}
                  </Badge>
                </div>

                <div className="grid gap-3 md:grid-cols-3">
                  <MetaCard label="Created" value={formatDate(activeDocument.createdAt)} />
                  <MetaCard label="Updated" value={formatDate(activeDocument.updatedAt)} />
                  <MetaCard label="External ID" value={truncate(activeDocument.sourceExternalId)} />
                </div>

                <div>
                  <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                    Preview
                  </p>
                  <div className="mt-4 space-y-5 text-sm leading-8 text-foreground">
                    {activeDocument.contentPlaintext
                      .split(/\n{2,}/)
                      .filter(Boolean)
                      .map((segment, index) => (
                        <p key={`${activeDocument.id}-${index}`}>
                          {highlightText(segment, search)}
                        </p>
                      ))}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </ResizablePanel>

        <ResizableHandle withHandle />

        <ResizablePanel defaultSize={25} minSize={20}>
          <div className="flex h-full flex-col">
            <div className="px-5 py-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Inspector
              </p>
              <h3 className="mt-2 text-lg font-semibold">Chunks, metadata, and related context</h3>
            </div>
            <Separator />
            <div className="min-h-0 flex-1 overflow-auto px-4 py-4">
              <div className="space-y-4">
                <Card className="p-4">
                  <div className="flex items-center gap-2 text-sm font-medium">
                    <Tags className="h-4 w-4 text-primary" />
                    Tags and source relations
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    {activeDocument.tags.length > 0 ? (
                      activeDocument.tags.map((tag) => (
                        <Badge key={tag} variant="outline">
                          {tag}
                        </Badge>
                      ))
                    ) : (
                      <p className="text-sm text-muted-foreground">No tags attached yet.</p>
                    )}
                  </div>
                </Card>

                <Card className="p-4">
                  <p className="text-sm font-medium">Chunk candidates</p>
                  <div className="mt-3 space-y-3">
                    {retrievalSegments.map((segment, index) => (
                      <div
                        key={`${activeDocument.id}-${index}`}
                        className="rounded-2xl border border-border bg-secondary/55 px-3 py-3"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                            Chunk {index + 1}
                          </p>
                          <Badge variant="secondary">Linked</Badge>
                        </div>
                        <p className="mt-2 text-sm leading-6 text-muted-foreground">
                          {highlightText(segment.slice(0, 180), search)}
                        </p>
                      </div>
                    ))}
                  </div>
                </Card>

                <Card className="p-4">
                  <p className="text-sm font-medium">Related documents</p>
                  <div className="mt-3 space-y-3">
                    {relatedDocuments.length > 0 ? (
                      relatedDocuments.map((document) => (
                        <button
                          key={document.id}
                          className="w-full rounded-2xl border border-border bg-secondary/55 px-3 py-3 text-left transition hover:border-primary/30"
                          onClick={() => handleSelectDocument(document.id)}
                          type="button"
                        >
                          <p className="text-sm font-medium">{document.title}</p>
                          <p className="mt-1 text-xs text-muted-foreground">
                            {document.sourceKind}
                          </p>
                        </button>
                      ))
                    ) : (
                      <p className="text-sm text-muted-foreground">
                        No closely related documents are visible in the current filter set.
                      </p>
                    )}
                  </div>
                </Card>
              </div>
            </div>
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}

function MetaCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-border bg-secondary/50 px-4 py-3">
      <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">{label}</p>
      <p className="mt-2 text-sm font-medium">{value}</p>
    </div>
  );
}

function formatDate(value: string | null) {
  if (!value) {
    return 'Unavailable';
  }

  return new Date(value).toLocaleString();
}

function truncate(value: string, maxLength = 18) {
  if (value.length <= maxLength) {
    return value;
  }

  return `${value.slice(0, maxLength)}…`;
}

function highlightText(text: string, query: string) {
  const normalizedQuery = query.trim();

  if (!normalizedQuery) {
    return text;
  }

  const parts = text.split(new RegExp(`(${escapeRegExp(normalizedQuery)})`, 'gi'));

  return parts.map((part, index) =>
    part.toLowerCase() === normalizedQuery.toLowerCase() ? (
      <mark
        key={`${part}-${index}`}
        className="rounded bg-primary/15 px-1 text-foreground"
      >
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
