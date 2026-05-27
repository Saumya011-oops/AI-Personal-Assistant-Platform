import { AlertCircle, CheckCircle2, Clock, Database, Loader2, RefreshCw, Wifi, XCircle } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { useGoogleConnectMutation } from '@/features/integrations/hooks/use-google-connect-mutation';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';
import { useNotionSyncMutation } from '@/features/integrations/hooks/use-notion-sync-mutation';
import { useObsidianScanMutation } from '@/features/integrations/hooks/use-obsidian-scan-mutation';

const statusIcon = (status: string) => {
  switch (status) {
    case 'connected':
      return <CheckCircle2 className="h-5 w-5 text-emerald-400" />;
    case 'syncing':
      return <Loader2 className="h-5 w-5 animate-spin text-indigo-400" />;
    case 'error':
      return <XCircle className="h-5 w-5 text-red-400" />;
    default:
      return <AlertCircle className="h-5 w-5 text-slate-500" />;
  }
};

const integrationIcon = (key: string) => {
  switch (key) {
    case 'notion':
      return <Database className="h-6 w-6 text-slate-300" />;
    case 'obsidian':
      return <Database className="h-6 w-6 text-purple-400" />;
    default:
      return <Wifi className="h-6 w-6 text-blue-400" />;
  }
};

export function IntegrationsPage() {
  const summaries = useIntegrationSummariesQuery();
  const notionSync = useNotionSyncMutation();
  const obsidianScan = useObsidianScanMutation();
  const googleConnect = useGoogleConnectMutation();

  if (summaries.isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="h-8 w-8 animate-spin text-slate-500" />
        <span className="ml-3 text-slate-400">Loading integrations…</span>
      </div>
    );
  }

  if (summaries.isError) {
    const errorObj = (summaries as any).error;
    const errMsg = errorObj instanceof Error
      ? errorObj.message
      : typeof errorObj === 'string'
        ? errorObj
        : JSON.stringify(errorObj, null, 2);
    return (
      <div className="rounded-2xl border border-red-500/30 bg-red-500/10 p-6">
        <div className="flex items-center gap-3">
          <XCircle className="h-6 w-6 text-red-400" />
          <div>
            <p className="font-semibold text-red-300">Failed to load integrations</p>
            <p className="mt-1 text-sm text-red-400/70 font-mono">
              {errMsg}
            </p>
          </div>
        </div>
        <Button className="mt-4" variant="secondary" onClick={() => summaries.refetch()}>
          <RefreshCw className="mr-2 h-4 w-4" /> Retry
        </Button>
      </div>
    );
  }

  const items = summaries.data ?? [];

  if (items.length === 0) {
    return (
      <div className="rounded-2xl border border-border/60 p-10 text-center">
        <AlertCircle className="mx-auto h-10 w-10 text-slate-500" />
        <p className="mt-4 text-slate-400">No integrations found. The database may still be initialising — try refreshing.</p>
        <Button className="mt-4" variant="secondary" onClick={() => summaries.refetch()}>
          <RefreshCw className="mr-2 h-4 w-4" /> Refresh
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">Integrations</h2>
        <Button variant="ghost" size="sm" onClick={() => summaries.refetch()}>
          <RefreshCw className="mr-2 h-4 w-4" /> Refresh
        </Button>
      </div>

      {items.map((integration) => (
        <Card key={integration.key}>
          <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
            <div className="flex items-center gap-4">
              <div className="rounded-xl bg-white/5 p-3">
                {integrationIcon(integration.key)}
              </div>
              <div>
                <div className="flex items-center gap-2">
                  <h3 className="text-lg font-semibold">{integration.label}</h3>
                  {statusIcon(integration.status)}
                </div>
                <p className="mt-0.5 text-sm text-slate-400 capitalize">
                  {integration.status.replaceAll('_', ' ')}
                  {integration.detail ? ` · ${integration.detail}` : ''}
                </p>
                {integration.lastSyncedAt && (
                  <p className="mt-0.5 flex items-center gap-1 text-xs text-slate-500">
                    <Clock className="h-3 w-3" />
                    Last synced: {new Date(integration.lastSyncedAt).toLocaleString()}
                  </p>
                )}
              </div>
            </div>

            <div className="flex gap-2">
              {integration.key === 'notion' ? (
                <Button
                  onClick={() => notionSync.mutate()}
                  variant="secondary"
                  disabled={notionSync.isPending}
                >
                  {notionSync.isPending ? (
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  ) : (
                    <RefreshCw className="mr-2 h-4 w-4" />
                  )}
                  {notionSync.isPending ? 'Syncing…' : 'Sync Notion'}
                </Button>
              ) : null}

              {integration.key === 'obsidian' ? (
                <Button
                  onClick={() => obsidianScan.mutate()}
                  variant="secondary"
                  disabled={obsidianScan.isPending}
                >
                  {obsidianScan.isPending ? (
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  ) : (
                    <Database className="mr-2 h-4 w-4" />
                  )}
                  {obsidianScan.isPending ? 'Scanning…' : 'Scan vault'}
                </Button>
              ) : null}

              {integration.key === 'google' ? (
                <Button
                  onClick={() => googleConnect.mutate()}
                  disabled={googleConnect.isPending}
                >
                  {googleConnect.isPending ? (
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  ) : (
                    <Wifi className="mr-2 h-4 w-4" />
                  )}
                  Connect Google
                </Button>
              ) : null}
            </div>
          </div>

          {/* Show error if sync failed */}
          {integration.key === 'notion' && notionSync.isError && (
            <div className="mt-3 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
              Sync failed: {String(notionSync.error)}
            </div>
          )}
          {integration.key === 'obsidian' && obsidianScan.isError && (
            <div className="mt-3 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
              Scan failed: {String(obsidianScan.error)}
            </div>
          )}
        </Card>
      ))}
    </div>
  );
}
