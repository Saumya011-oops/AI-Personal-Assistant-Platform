import {
  ArrowRight,
  BookOpenText,
  CheckCircle2,
  FolderSync,
  HardDriveDownload,
  ShieldCheck,
  Sparkles,
} from 'lucide-react';
import { useMemo } from 'react';
import { useNavigate } from 'react-router-dom';

import { EmptyState } from '@/components/states/empty-state';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';
import { useAppStatusQuery } from '@/features/dashboard/hooks/use-app-status-query';
import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';

type ActivityTone = 'success' | 'outline' | 'secondary' | 'destructive';

export function HomePage() {
  const navigate = useNavigate();
  const appStatus = useAppStatusQuery();
  const integrations = useIntegrationSummariesQuery();
  const documents = useDocumentsQuery();

  const integrationItems = integrations.data;
  const documentItems = documents.data;
  const connectedIntegrations = (integrationItems ?? []).filter(
    (item) => item.status === 'connected',
  );

  const retrievalReady =
    Boolean(appStatus.data?.databaseReady) &&
    Boolean(appStatus.data?.rustBackendAvailable) &&
    (documentItems?.length ?? 0) > 0;

  const activity = useMemo(() => {
    const sourceEvents = (integrationItems ?? []).map((item) => ({
      id: item.key,
      title: item.label,
      detail: item.detail ?? 'Ready for sync and retrieval.',
      time: item.lastSyncedAt,
      tone: (
        item.status === 'connected'
          ? 'success'
          : item.status === 'error'
            ? 'destructive'
            : 'outline'
      ) as ActivityTone,
    }));

    const documentEvents = (documentItems ?? [])
      .slice(0, 3)
      .map((document) => ({
        id: document.id,
        title: document.title,
        detail: `Indexed from ${document.sourceKind}`,
        time: document.updatedAt,
        tone: 'secondary' as const,
      }));

    return [...sourceEvents, ...documentEvents]
      .sort((left, right) => {
        const leftTime = left.time ? new Date(left.time).getTime() : 0;
        const rightTime = right.time ? new Date(right.time).getTime() : 0;
        return rightTime - leftTime;
      })
      .slice(0, 5);
  }, [documentItems, integrationItems]);

  const quickActions = [
    {
      label: 'Open Assistant',
      detail: 'Start a grounded conversation',
      run: () => navigate('/assistant'),
    },
    {
      label: 'Browse Knowledge',
      detail: 'Inspect indexed documents',
      run: () => navigate('/documents'),
    },
    {
      label: 'Manage Integrations',
      detail: 'Check source and sync readiness',
      run: () => navigate('/integrations'),
    },
  ];

  return (
    <div className="mx-auto flex max-w-[1480px] flex-col gap-6">
      <section className="grid gap-6 xl:grid-cols-[1.25fr_0.75fr]">
        <Card className="p-8">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="outline">Workspace overview</Badge>
            <Badge variant="secondary">Desktop knowledge system</Badge>
          </div>
          <h2 className="mt-5 max-w-3xl text-4xl font-semibold tracking-tight">
            A calm, local-first workspace for grounded AI work across your knowledge sources.
          </h2>
          <p className="mt-4 max-w-2xl text-base leading-7 text-muted-foreground">
            Keep the landing experience focused on readiness and next actions, then move into
            chat or document exploration only when you need it.
          </p>

          <div className="mt-8 flex flex-wrap gap-3">
            {quickActions.map((action) => (
              <Button
                key={action.label}
                className="h-11 rounded-2xl px-4"
                onClick={action.run}
                variant={action.label === 'Open Assistant' ? 'default' : 'secondary'}
              >
                {action.label}
                <ArrowRight className="ml-2 h-4 w-4" />
              </Button>
            ))}
          </div>
        </Card>

        <Card className="p-6">
          <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
            System readiness
          </p>
          <div className="mt-5 space-y-4">
            <ReadinessRow
              label="Retrieval pipeline"
              value={retrievalReady ? 'Ready' : 'Needs setup'}
              tone={retrievalReady ? 'success' : 'outline'}
              detail={`${documentItems?.length ?? 0} indexed document${documentItems?.length === 1 ? '' : 's'}`}
            />
            <ReadinessRow
              label="Source integrations"
              value={`${connectedIntegrations.length}/${integrationItems?.length ?? 0} connected`}
              tone={connectedIntegrations.length > 0 ? 'success' : 'outline'}
              detail="Notion, Obsidian, and Google foundation"
            />
            <ReadinessRow
              label="Desktop backend"
              value={appStatus.data?.rustBackendAvailable ? 'Live' : 'Pending'}
              tone={appStatus.data?.rustBackendAvailable ? 'success' : 'outline'}
              detail={appStatus.data?.environment ?? 'development'}
            />
          </div>
        </Card>
      </section>

      <section className="grid gap-6 xl:grid-cols-[0.95fr_1.05fr]">
        <Card className="p-0">
          <div className="px-6 py-5">
            <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
              Workspace health
            </p>
            <h3 className="mt-2 text-xl font-semibold">High-level readiness without dashboard noise</h3>
          </div>
          <Separator />
          <div className="grid gap-0 md:grid-cols-3">
            <MetricCell
              icon={BookOpenText}
              label="Knowledge base"
              value={`${documentItems?.length ?? 0}`}
              detail="Normalized documents indexed locally"
            />
            <MetricCell
              icon={FolderSync}
              label="Sync health"
              value={`${connectedIntegrations.length}`}
              detail="Connected sources available for refresh"
            />
            <MetricCell
              icon={ShieldCheck}
              label="Offline posture"
              value={appStatus.data?.databaseReady ? 'Stable' : 'Pending'}
              detail="Local SQLite layer available to the assistant"
              borderless
            />
          </div>
        </Card>

        <Card className="p-0">
          <div className="px-6 py-5">
            <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
              Quick actions
            </p>
            <h3 className="mt-2 text-xl font-semibold">Start the next useful workflow</h3>
          </div>
          <Separator />
          <div className="grid gap-3 p-5 md:grid-cols-3">
            <ActionCard
              icon={Sparkles}
              title="Ask with context"
              description="Move into the dedicated assistant workspace with citations and source awareness."
              onClick={() => navigate('/assistant')}
            />
            <ActionCard
              icon={HardDriveDownload}
              title="Inspect your corpus"
              description="Review indexed source material in the split-view knowledge explorer."
              onClick={() => navigate('/documents')}
            />
            <ActionCard
              icon={FolderSync}
              title="Review sync status"
              description="Check integration readiness, sync actions, and future adapters."
              onClick={() => navigate('/integrations')}
            />
          </div>
        </Card>
      </section>

      <section className="grid gap-6 xl:grid-cols-[0.8fr_1.2fr]">
        <Card className="p-0">
          <div className="px-6 py-5">
            <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
              Sync health
            </p>
            <h3 className="mt-2 text-xl font-semibold">Source readiness at a glance</h3>
          </div>
          <Separator />
          <div className="space-y-3 p-5">
            {(integrationItems?.length ?? 0) === 0 ? (
              <EmptyState
                description="No source adapters have reported status yet."
                title="Integrations are not loaded"
              />
            ) : (
              (integrationItems ?? []).map((integration) => (
                <div
                  key={integration.key}
                  className="rounded-2xl border border-border bg-secondary/50 px-4 py-4"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <p className="text-sm font-medium">{integration.label}</p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {integration.detail ?? 'Awaiting activity'}
                      </p>
                    </div>
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
                </div>
              ))
            )}
          </div>
        </Card>

        <Card className="p-0">
          <div className="px-6 py-5">
            <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">
              Recent activity
            </p>
            <h3 className="mt-2 text-xl font-semibold">What changed in the workspace</h3>
          </div>
          <Separator />
          <div className="space-y-3 p-5">
            {activity.length === 0 ? (
              <EmptyState
                description="Connect a source or sync documents to populate activity."
                title="No recent activity yet"
              />
            ) : (
              activity.map((item) => (
                <div
                  key={item.id}
                  className="flex items-start gap-3 rounded-2xl border border-border bg-secondary/50 px-4 py-4"
                >
                  <CheckCircle2 className="mt-0.5 h-4 w-4 text-primary" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <p className="truncate text-sm font-medium">{item.title}</p>
                      <Badge variant={item.tone}>{item.time ? 'Updated' : 'Ready'}</Badge>
                    </div>
                    <p className="mt-1 text-sm text-muted-foreground">{item.detail}</p>
                    <p className="mt-2 text-xs text-muted-foreground">
                      {item.time ? new Date(item.time).toLocaleString() : 'Awaiting first sync activity'}
                    </p>
                  </div>
                </div>
              ))
            )}
          </div>
        </Card>
      </section>
    </div>
  );
}

function ReadinessRow({
  label,
  value,
  detail,
  tone,
}: {
  label: string;
  value: string;
  detail: string;
  tone: 'success' | 'outline' | 'secondary' | 'destructive' | 'warning';
}) {
  return (
    <div className="rounded-2xl border border-border bg-secondary/45 px-4 py-4">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm font-medium">{label}</p>
        <Badge variant={tone}>{value}</Badge>
      </div>
      <p className="mt-2 text-sm text-muted-foreground">{detail}</p>
    </div>
  );
}

function MetricCell({
  icon: Icon,
  label,
  value,
  detail,
  borderless,
}: {
  icon: typeof BookOpenText;
  label: string;
  value: string;
  detail: string;
  borderless?: boolean;
}) {
  return (
    <div className={`p-6 ${borderless ? '' : 'border-b border-border md:border-b-0 md:border-r'}`}>
      <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-secondary/70">
        <Icon className="h-5 w-5 text-primary" />
      </div>
      <p className="mt-4 text-xs uppercase tracking-[0.2em] text-muted-foreground">{label}</p>
      <p className="mt-2 text-3xl font-semibold">{value}</p>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">{detail}</p>
    </div>
  );
}

function ActionCard({
  icon: Icon,
  title,
  description,
  onClick,
}: {
  icon: typeof Sparkles;
  title: string;
  description: string;
  onClick: () => void;
}) {
  return (
    <button
      className="rounded-3xl border border-border bg-secondary/45 p-5 text-left transition hover:border-primary/30 hover:bg-secondary/70"
      onClick={onClick}
      type="button"
    >
      <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-background/70">
        <Icon className="h-5 w-5 text-primary" />
      </div>
      <p className="mt-4 text-base font-semibold">{title}</p>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">{description}</p>
    </button>
  );
}
