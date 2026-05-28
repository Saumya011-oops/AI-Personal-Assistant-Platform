import {
  CheckCircle2,
  CloudCog,
  Database,
  Loader2,
  Mail,
  RefreshCw,
} from 'lucide-react';

import { ErrorState } from '@/components/states/error-state';
import { LoadingState } from '@/components/states/loading-state';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';
import { useGoogleConnectMutation } from '@/features/integrations/hooks/use-google-connect-mutation';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';
import { useNotionSyncMutation } from '@/features/integrations/hooks/use-notion-sync-mutation';
import { useObsidianScanMutation } from '@/features/integrations/hooks/use-obsidian-scan-mutation';

const futureIntegrations = [
  { label: 'Gmail', icon: Mail, description: 'Thread summaries, inbox retrieval, and grounded email actions.' },
  { label: 'Calendar', icon: CloudCog, description: 'Time-aware retrieval and schedule-assisted planning.' },
];

export function IntegrationsPage() {
  const summaries = useIntegrationSummariesQuery();
  const notionSync = useNotionSyncMutation();
  const obsidianScan = useObsidianScanMutation();
  const googleConnect = useGoogleConnectMutation();

  if (summaries.isLoading) {
    return <LoadingState label="Loading integration status and sync health." />;
  }

  if (summaries.isError) {
    return (
      <ErrorState
        description="The desktop app could not read source integration status."
        onRetry={() => summaries.refetch()}
        title="Failed to load integrations"
      />
    );
  }

  return (
    <div className="space-y-4">
      <Card className="p-5">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
              Source Control
            </p>
            <h2 className="mt-1 text-xl font-semibold">Integrations and sync health</h2>
            <p className="mt-2 text-sm text-muted-foreground">
              Connect knowledge systems, trigger indexed syncs, and monitor the readiness of future source adapters.
            </p>
          </div>
          <Button onClick={() => summaries.refetch()} size="sm" variant="secondary">
            <RefreshCw className="mr-2 h-4 w-4" />
            Refresh
          </Button>
        </div>
      </Card>

      <div className="grid gap-4 xl:grid-cols-[1.15fr_0.85fr]">
        <div className="space-y-4">
          {(summaries.data ?? []).map((integration) => {
            const isNotion = integration.key === 'notion';
            const isObsidian = integration.key === 'obsidian';
            const isGoogle = integration.key === 'google';

            return (
              <Card key={integration.key} className="p-5">
                <div className="flex flex-wrap items-start justify-between gap-4">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-3">
                      <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-secondary">
                        {isNotion || isObsidian ? (
                          <Database className="h-5 w-5 text-primary" />
                        ) : (
                          <CloudCog className="h-5 w-5 text-primary" />
                        )}
                      </div>
                      <div>
                        <div className="flex items-center gap-2">
                          <h3 className="text-lg font-semibold">{integration.label}</h3>
                          <Badge
                            variant={
                              integration.status === 'connected'
                                ? 'success'
                                : integration.status === 'error'
                                  ? 'destructive'
                                  : integration.status === 'syncing'
                                    ? 'warning'
                                    : 'outline'
                            }
                          >
                            {integration.status}
                          </Badge>
                        </div>
                        <p className="mt-1 text-sm text-muted-foreground">
                          {integration.detail ?? 'Ready to configure and sync.'}
                        </p>
                      </div>
                    </div>

                    {integration.lastSyncedAt ? (
                      <p className="mt-4 text-xs text-muted-foreground">
                        Last sync: {new Date(integration.lastSyncedAt).toLocaleString()}
                      </p>
                    ) : null}
                  </div>

                  <div className="flex gap-2">
                    {isNotion ? (
                      <Button
                        disabled={notionSync.isPending}
                        onClick={() => notionSync.mutate()}
                        variant="secondary"
                      >
                        {notionSync.isPending ? (
                          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        ) : (
                          <RefreshCw className="mr-2 h-4 w-4" />
                        )}
                        {notionSync.isPending ? 'Syncing' : 'Sync'}
                      </Button>
                    ) : null}
                    {isObsidian ? (
                      <Button
                        disabled={obsidianScan.isPending}
                        onClick={() => obsidianScan.mutate()}
                        variant="secondary"
                      >
                        {obsidianScan.isPending ? (
                          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        ) : (
                          <RefreshCw className="mr-2 h-4 w-4" />
                        )}
                        {obsidianScan.isPending ? 'Scanning' : 'Scan vault'}
                      </Button>
                    ) : null}
                    {isGoogle ? (
                      <Button
                        disabled={googleConnect.isPending}
                        onClick={() => googleConnect.mutate()}
                      >
                        {googleConnect.isPending ? (
                          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        ) : (
                          <CheckCircle2 className="mr-2 h-4 w-4" />
                        )}
                        Connect
                      </Button>
                    ) : null}
                  </div>
                </div>
              </Card>
            );
          })}
        </div>

        <Card className="p-0">
          <div className="px-5 py-4">
            <p className="text-sm font-medium">Roadmap adapters</p>
            <p className="mt-1 text-xs text-muted-foreground">
              Future integrations already accounted for in the information architecture.
            </p>
          </div>
          <Separator />
          <div className="space-y-3 p-5">
            {futureIntegrations.map((integration) => (
              <div
                key={integration.label}
                className="rounded-2xl border border-border bg-secondary/55 p-4"
              >
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-background/60">
                    <integration.icon className="h-5 w-5 text-primary" />
                  </div>
                  <div>
                    <p className="text-sm font-medium">{integration.label}</p>
                    <p className="mt-1 text-xs leading-5 text-muted-foreground">
                      {integration.description}
                    </p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </Card>
      </div>
    </div>
  );
}
