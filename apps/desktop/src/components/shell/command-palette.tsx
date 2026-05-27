import { BookOpen, Database, FileText, RefreshCw, Search, Settings, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';
import { useNotionSyncMutation } from '@/features/integrations/hooks/use-notion-sync-mutation';
import { useObsidianScanMutation } from '@/features/integrations/hooks/use-obsidian-scan-mutation';
import { useUiStore } from '@/stores/ui-store';

const sourceIcon = (kind: string) => {
  switch (kind) {
    case 'notion': return <BookOpen className="h-4 w-4 text-indigo-400" />;
    case 'obsidian': return <Database className="h-4 w-4 text-purple-400" />;
    default: return <FileText className="h-4 w-4 text-slate-400" />;
  }
};

const STATIC_ACTIONS = [
  { id: 'integrations', label: 'Open integrations', icon: <Settings className="h-4 w-4 text-slate-400" />, path: '/integrations' },
  { id: 'settings', label: 'Go to settings', icon: <Settings className="h-4 w-4 text-slate-400" />, path: '/settings' },
  { id: 'documents', label: 'Browse documents', icon: <FileText className="h-4 w-4 text-slate-400" />, path: '/documents' },
];

export function CommandPalette() {
  const isOpen = useUiStore(state => state.commandPaletteOpen);
  const close = useUiStore(state => state.closeCommandPalette);
  const navigate = useNavigate();
  const notionSync = useNotionSyncMutation();
  const obsidianScan = useObsidianScanMutation();
  const docsQuery = useDocumentsQuery();
  const [query, setQuery] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus input when opened
  useEffect(() => {
    if (isOpen) {
      setTimeout(() => inputRef.current?.focus(), 50);
      setQuery('');
    }
  }, [isOpen]);

  const allDocs = docsQuery.data ?? [];
  const q = query.toLowerCase().trim();

  // Filter docs by query
  const matchedDocs = q.length >= 1
    ? allDocs.filter(d =>
        d.title.toLowerCase().includes(q) ||
        d.contentPlaintext.toLowerCase().includes(q)
      ).slice(0, 6)
    : [];

  // Filter static actions by query
  const matchedActions = q
    ? STATIC_ACTIONS.filter(a => a.label.toLowerCase().includes(q))
    : STATIC_ACTIONS;

  // Command actions shown when no query
  const syncActions = [
    {
      id: 'notion-sync',
      label: notionSync.isPending ? 'Syncing Notion…' : 'Run Notion sync',
      icon: <RefreshCw className={`h-4 w-4 ${notionSync.isPending ? 'animate-spin text-indigo-400' : 'text-slate-400'}`} />,
      onClick: () => { notionSync.mutate(); close(); },
      disabled: notionSync.isPending,
    },
    {
      id: 'obsidian-scan',
      label: obsidianScan.isPending ? 'Scanning vault…' : 'Scan Obsidian vault',
      icon: <Database className={`h-4 w-4 ${obsidianScan.isPending ? 'animate-spin text-purple-400' : 'text-slate-400'}`} />,
      onClick: () => { obsidianScan.mutate(); close(); },
      disabled: obsidianScan.isPending,
    },
  ];

  const filteredSyncActions = q
    ? syncActions.filter(a => a.label.toLowerCase().includes(q))
    : syncActions;

  const hasResults = matchedDocs.length > 0 || matchedActions.length > 0 || filteredSyncActions.length > 0;

  return (
    <Dialog open={isOpen} onOpenChange={open => !open && close()}>
      <DialogContent className="max-w-2xl border-border/70 bg-[#0c1117] p-0 shadow-panel">
        {/* Search input */}
        <div className="border-b border-border/60 px-4 py-3">
          <div className="flex items-center gap-3 rounded-xl bg-white/5 px-4 py-2.5">
            <Search className="h-4 w-4 shrink-0 text-slate-400" />
            <input
              ref={inputRef}
              className="flex-1 bg-transparent text-sm text-slate-200 placeholder-slate-500 outline-none"
              placeholder="Search documents, actions…"
              value={query}
              onChange={e => setQuery(e.target.value)}
            />
            {query && (
              <button onClick={() => setQuery('')} className="text-slate-500 hover:text-slate-300">
                <X className="h-4 w-4" />
              </button>
            )}
          </div>
        </div>

        <div className="max-h-[420px] overflow-y-auto p-3 space-y-4">
          {/* Document results */}
          {matchedDocs.length > 0 && (
            <div>
              <p className="mb-1.5 px-2 text-xs font-medium uppercase tracking-wider text-slate-600">Documents</p>
              <div className="space-y-1">
                {matchedDocs.map(doc => (
                  <button
                    key={doc.id}
                    className="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left hover:bg-white/5"
                    onClick={() => { navigate('/documents'); close(); }}
                  >
                    {sourceIcon(doc.sourceKind)}
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium text-slate-200">{doc.title}</p>
                      <p className="truncate text-xs text-slate-500">{doc.contentPlaintext.slice(0, 80)}</p>
                    </div>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Sync actions */}
          {filteredSyncActions.length > 0 && (
            <div>
              <p className="mb-1.5 px-2 text-xs font-medium uppercase tracking-wider text-slate-600">Sync</p>
              <div className="space-y-1">
                {filteredSyncActions.map(action => (
                  <Button
                    key={action.id}
                    className="w-full justify-start gap-3 rounded-xl"
                    variant="ghost"
                    onClick={action.onClick}
                    disabled={action.disabled}
                  >
                    {action.icon}
                    {action.label}
                  </Button>
                ))}
              </div>
            </div>
          )}

          {/* Navigation actions */}
          {matchedActions.length > 0 && (
            <div>
              <p className="mb-1.5 px-2 text-xs font-medium uppercase tracking-wider text-slate-600">Navigate</p>
              <div className="space-y-1">
                {matchedActions.map(action => (
                  <Button
                    key={action.id}
                    className="w-full justify-start gap-3 rounded-xl"
                    variant="ghost"
                    onClick={() => { navigate(action.path); close(); }}
                  >
                    {action.icon}
                    {action.label}
                  </Button>
                ))}
              </div>
            </div>
          )}

          {q && !hasResults && (
            <p className="px-3 py-6 text-center text-sm text-slate-500">
              No results for "{query}"
            </p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
