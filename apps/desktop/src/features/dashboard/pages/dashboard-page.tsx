import { Activity, Bot, Database, FileText, RefreshCw, ShieldCheck } from 'lucide-react';
import { useNavigate } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { useAppStatusQuery } from '@/features/dashboard/hooks/use-app-status-query';
import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';
import { useNotionSyncMutation } from '@/features/integrations/hooks/use-notion-sync-mutation';
import { useObsidianScanMutation } from '@/features/integrations/hooks/use-obsidian-scan-mutation';

export function DashboardPage() {
  const navigate = useNavigate();
  const appStatus = useAppStatusQuery();
  const integrations = useIntegrationSummariesQuery();
  const documents = useDocumentsQuery();
  const notionSync = useNotionSyncMutation();
  const obsidianScan = useObsidianScanMutation();

  const notionDocs = (documents.data ?? []).filter(d => d.sourceKind === 'notion').length;
  const obsidianDocs = (documents.data ?? []).filter(d => d.sourceKind === 'obsidian').length;
  const totalDocs = (documents.data ?? []).length;

  const stats = [
    {
      label: 'App Environment',
      value: appStatus.data?.environment ?? 'Loading…',
      icon: Activity,
      color: 'text-sky-300',
    },
    {
      label: 'Backend',
      value: appStatus.data?.rustBackendAvailable ? 'Connected' : (appStatus.isLoading ? 'Loading…' : 'Disconnected'),
      icon: Bot,
      color: appStatus.data?.rustBackendAvailable ? 'text-emerald-400' : 'text-sky-300',
    },
    {
      label: 'Database',
      value: appStatus.data?.databaseReady ? 'Ready' : (appStatus.isLoading ? 'Loading…' : 'Error'),
      icon: Database,
      color: appStatus.data?.databaseReady ? 'text-emerald-400' : 'text-sky-300',
    },
    {
      label: 'Documents',
      value: documents.isLoading ? 'Loading…' : `${totalDocs} indexed`,
      icon: FileText,
      color: totalDocs > 0 ? 'text-indigo-400' : 'text-sky-300',
    },
  ];

  const connectedIntegrations = (integrations.data ?? []).filter(i => i.status === 'connected');

  return (
    <div className="space-y-6">
      {/* Stats row */}
      <section className="grid gap-4 lg:grid-cols-4">
        {stats.map(({ label, value, icon: Icon, color }) => (
          <Card key={label}>
            <div className="flex items-center justify-between">
              <p className="text-sm text-slate-400">{label}</p>
              <Icon className={`h-4 w-4 ${color}`} />
            </div>
            <p className="mt-4 text-2xl font-semibold">{value}</p>
          </Card>
        ))}
      </section>

      {/* Middle row */}
      <section className="grid gap-4 xl:grid-cols-[1.2fr_0.8fr]">
        {/* Source breakdown */}
        <Card>
          <p className="text-sm uppercase tracking-[0.24em] text-slate-500">Knowledge Base</p>
          <div className="mt-4 space-y-3">
            {[
              { label: 'Notion pages', count: notionDocs, color: 'bg-indigo-500', action: () => notionSync.mutate(), actionLabel: notionSync.isPending ? 'Syncing…' : 'Sync now' },
              { label: 'Obsidian notes', count: obsidianDocs, color: 'bg-purple-500', action: () => obsidianScan.mutate(), actionLabel: obsidianScan.isPending ? 'Scanning…' : 'Scan now' },
            ].map(({ label, count, color, action, actionLabel }) => (
              <div key={label} className="flex items-center gap-3">
                <div className={`h-2 w-2 rounded-full ${color}`} />
                <span className="flex-1 text-sm text-slate-300">{label}</span>
                <span className="text-sm font-semibold tabular-nums">{count}</span>
                <button
                  onClick={action}
                  className="text-xs text-slate-500 hover:text-indigo-400"
                >
                  {actionLabel}
                </button>
              </div>
            ))}
            {totalDocs > 0 && (
              <div className="mt-3 border-t border-border/60 pt-3">
                <Button variant="secondary" className="w-full" onClick={() => navigate('/documents')}>
                  <FileText className="mr-2 h-4 w-4" /> Browse all {totalDocs} documents
                </Button>
              </div>
            )}
          </div>
        </Card>

        {/* Integrations status */}
        <Card>
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold">Integrations</h3>
            <Button variant="ghost" size="sm" onClick={() => navigate('/integrations')}>
              <ShieldCheck className="mr-1 h-4 w-4" /> Manage
            </Button>
          </div>
          <div className="mt-3 space-y-2">
            {(integrations.data ?? []).map(integration => (
              <div key={integration.key} className="flex items-center gap-3 rounded-xl bg-white/5 px-3 py-2">
                <span className={`h-2 w-2 rounded-full ${
                  integration.status === 'connected' ? 'bg-emerald-400' :
                  integration.status === 'error' ? 'bg-red-400' :
                  integration.status === 'syncing' ? 'bg-indigo-400 animate-pulse' :
                  'bg-slate-600'
                }`} />
                <span className="flex-1 text-sm capitalize">{integration.label}</span>
                <span className="text-xs text-slate-500 capitalize">{integration.status.replace('_', ' ')}</span>
              </div>
            ))}
            {integrations.isLoading && (
              <p className="text-sm text-slate-500">Loading integrations…</p>
            )}
          </div>
        </Card>
      </section>

      {/* Quick actions */}
      <section>
        <Card>
          <p className="text-sm uppercase tracking-[0.24em] text-slate-500">Quick Actions</p>
          <div className="mt-4 flex flex-wrap gap-3">
            <Button variant="secondary" onClick={() => notionSync.mutate()} disabled={notionSync.isPending}>
              <RefreshCw className={`mr-2 h-4 w-4 ${notionSync.isPending ? 'animate-spin' : ''}`} />
              {notionSync.isPending ? 'Syncing Notion…' : 'Sync Notion'}
            </Button>
            <Button variant="secondary" onClick={() => obsidianScan.mutate()} disabled={obsidianScan.isPending}>
              <RefreshCw className={`mr-2 h-4 w-4 ${obsidianScan.isPending ? 'animate-spin' : ''}`} />
              {obsidianScan.isPending ? 'Scanning vault…' : 'Scan Obsidian Vault'}
            </Button>
            <Button variant="secondary" onClick={() => navigate('/documents')}>
              <FileText className="mr-2 h-4 w-4" /> Browse Documents
            </Button>
          </div>
        </Card>
      </section>
    </div>
  );
}
