import {
  ArrowUpRight,
  FolderSync,
  Mic,
  Paperclip,
  Search,
  Sparkles,
  TimerReset,
} from 'lucide-react';

import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Separator } from '@/components/ui/separator';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useAppStatusQuery } from '@/features/dashboard/hooks/use-app-status-query';
import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';

const draftMessages = [
  {
    role: 'user',
    content: 'Summarize everything we know about the current MVP foundation phase and identify missing infrastructure.',
  },
  {
    role: 'assistant',
    content:
      'The platform foundation is in place across Tauri, React, shared contracts, SQLite, and source integrations. Remaining work is mainly operational: complete live auth validation, harden sync retries, and wire future retrieval services behind the existing backend boundaries.',
  },
  {
    role: 'assistant',
    content:
      'I can also break this into architecture risks, implementation gaps, or a next-week execution checklist if you want a sharper operational view.',
  },
];

export function DashboardPage() {
  const appStatus = useAppStatusQuery();
  const integrations = useIntegrationSummariesQuery();
  const documents = useDocumentsQuery();

  const totalDocuments = (documents.data ?? []).length;
  const integrationItems = integrations.data ?? [];
  const connectedIntegrations = integrationItems.filter(
    (item) => item.status === 'connected',
  ).length;

  const citationItems = (documents.data ?? []).slice(0, 5);
  const liveSignals = [
    {
      label: 'Documents',
      value: documents.isLoading ? 'Loading' : `${totalDocuments}`,
      hint: 'Indexed knowledge records',
    },
    {
      label: 'Integrations',
      value: integrations.isLoading ? 'Loading' : `${connectedIntegrations}/${integrationItems.length}`,
      hint: 'Connected source systems',
    },
    {
      label: 'Backend',
      value: appStatus.data?.rustBackendAvailable ? 'Live' : 'Pending',
      hint: 'Tauri + Rust command bridge',
    },
  ];

  return (
    <div className="flex h-[calc(100vh-6.25rem)] min-h-[760px] flex-col gap-4">
      <section className="grid gap-4 xl:grid-cols-[1.1fr_0.9fr]">
        <Card className="p-6">
          <div className="flex items-start justify-between gap-4">
            <div>
              <div className="flex items-center gap-2">
                <Badge variant="success">Context-aware assistant</Badge>
                <Badge variant="outline">Desktop-first</Badge>
              </div>
              <h2 className="mt-4 text-2xl font-semibold tracking-tight">
                Multi-source assistant workspace for retrieval, synthesis, and action
              </h2>
              <p className="mt-3 max-w-2xl text-sm leading-6 text-muted-foreground">
                Built for fast keyboard-driven work: chat with grounded citations,
                inspect indexed knowledge, monitor sync health, and keep local-first
                context visible while you think.
              </p>
            </div>
            <div className="rounded-2xl border border-border bg-secondary/70 px-3 py-2 text-right">
              <p className="text-[11px] uppercase tracking-[0.24em] text-muted-foreground">
                Environment
              </p>
              <p className="mt-1 text-sm font-semibold">
                {appStatus.data?.environment ?? 'development'}
              </p>
            </div>
          </div>
        </Card>

        <div className="grid gap-4 md:grid-cols-3 xl:grid-cols-3">
          {liveSignals.map((signal) => (
            <Card key={signal.label} className="p-5">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                {signal.label}
              </p>
              <p className="mt-3 text-2xl font-semibold">{signal.value}</p>
              <p className="mt-1 text-sm text-muted-foreground">{signal.hint}</p>
            </Card>
          ))}
        </div>
      </section>

      <ResizablePanelGroup
        className="min-h-0 flex-1 rounded-[28px] border border-border bg-card/70"
        direction="horizontal"
      >
        <ResizablePanel defaultSize={22} minSize={18}>
          <div className="flex h-full flex-col">
            <div className="px-5 py-4">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                    Queues
                  </p>
                  <h3 className="mt-1 text-sm font-semibold">Priority workspaces</h3>
                </div>
                <Button size="icon" variant="ghost">
                  <Search className="h-4 w-4" />
                </Button>
              </div>
            </div>
            <Separator />
            <ScrollArea className="flex-1 px-3 pb-4">
              <div className="space-y-2 pt-3">
                {[
                  {
                    title: 'MVP foundation review',
                    source: 'Notion + Obsidian',
                    active: true,
                    state: 'Grounded',
                  },
                  {
                    title: 'Sync diagnostics',
                    source: 'Integration telemetry',
                    active: false,
                    state: 'Monitor',
                  },
                  {
                    title: 'Unread source changes',
                    source: 'Vault + docs feed',
                    active: false,
                    state: 'Queued',
                  },
                ].map((item) => (
                  <button
                    key={item.title}
                    className={`w-full rounded-2xl border px-4 py-3 text-left transition ${
                      item.active
                        ? 'border-primary/30 bg-primary/10'
                        : 'border-transparent bg-secondary/60 hover:border-border'
                    }`}
                    type="button"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <p className="text-sm font-medium">{item.title}</p>
                        <p className="mt-1 text-xs text-muted-foreground">{item.source}</p>
                      </div>
                      <Badge variant={item.active ? 'success' : 'outline'}>
                        {item.state}
                      </Badge>
                    </div>
                  </button>
                ))}
              </div>

              <div className="mt-5 rounded-3xl border border-border bg-secondary/50 p-4">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <FolderSync className="h-4 w-4 text-primary" />
                  Live source health
                </div>
                <div className="mt-3 space-y-2">
                  {integrationItems.map((integration) => (
                    <div
                      key={integration.key}
                      className="flex items-center justify-between rounded-2xl bg-background/40 px-3 py-2"
                    >
                      <div>
                        <p className="text-sm">{integration.label}</p>
                        <p className="text-xs text-muted-foreground">
                          {integration.detail ?? 'Awaiting activity'}
                        </p>
                      </div>
                      <Badge
                        variant={
                          integration.status === 'connected'
                            ? 'success'
                            : integration.status === 'error'
                              ? 'destructive'
                              : 'outline'
                        }
                      >
                        {integration.status}
                      </Badge>
                    </div>
                  ))}
                </div>
              </div>
            </ScrollArea>
          </div>
        </ResizablePanel>

        <ResizableHandle withHandle />

        <ResizablePanel defaultSize={50} minSize={36}>
          <div className="flex h-full flex-col">
            <div className="flex items-center justify-between px-5 py-4">
              <div>
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Assistant
                </p>
                <h3 className="mt-1 text-sm font-semibold">Grounded response session</h3>
              </div>
              <div className="flex items-center gap-2">
                <Badge variant="secondary">Streaming-ready</Badge>
                <Badge variant="outline">Citations on</Badge>
              </div>
            </div>
            <Separator />
            <ScrollArea className="flex-1 px-5 py-4">
              <div className="space-y-4">
                {draftMessages.map((message, index) => (
                  <div
                    key={`${message.role}-${index}`}
                    className={`flex gap-3 ${message.role === 'assistant' ? '' : 'justify-end'}`}
                  >
                    {message.role === 'assistant' && (
                      <Avatar className="mt-0.5 h-8 w-8">
                        <AvatarFallback>AI</AvatarFallback>
                      </Avatar>
                    )}
                    <div
                      className={`max-w-[82%] rounded-3xl px-4 py-3 ${
                        message.role === 'assistant'
                          ? 'border border-border bg-secondary/65'
                          : 'bg-primary text-primary-foreground'
                      }`}
                    >
                      <p className="text-sm leading-6">{message.content}</p>
                    </div>
                  </div>
                ))}

                <div className="rounded-3xl border border-dashed border-border bg-background/50 px-4 py-3">
                  <div className="flex items-center gap-2 text-sm font-medium">
                    <Sparkles className="h-4 w-4 text-primary" />
                    Response pipeline architecture
                  </div>
                  <div className="mt-3 grid gap-2 md:grid-cols-3">
                    {[
                      'Prompt composer with slash actions',
                      'Streaming answer pane with citations',
                      'Post-response follow-up suggestions',
                    ].map((item) => (
                      <div
                        key={item}
                        className="rounded-2xl border border-border bg-secondary/60 px-3 py-3 text-sm text-muted-foreground"
                      >
                        {item}
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </ScrollArea>
            <Separator />
            <div className="px-5 py-4">
              <div className="rounded-[22px] border border-border bg-background/60 p-3">
                <div className="flex items-end gap-3">
                  <div className="flex flex-1 flex-col gap-3">
                    <Input
                      className="h-12 rounded-2xl border-0 bg-transparent px-1 text-sm shadow-none focus-visible:ring-0"
                      placeholder="Ask about your synced knowledge, implementation gaps, or source evidence…"
                    />
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                      <Button size="icon" variant="ghost">
                        <Paperclip className="h-4 w-4" />
                      </Button>
                      <Button size="icon" variant="ghost">
                        <Mic className="h-4 w-4" />
                      </Button>
                      <span>Enter to send</span>
                    </div>
                  </div>
                  <Button className="h-12 rounded-2xl px-4" size="lg">
                    Ask assistant
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </ResizablePanel>

        <ResizableHandle withHandle />

        <ResizablePanel defaultSize={28} minSize={22}>
          <div className="flex h-full flex-col">
            <div className="px-5 py-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Context rail
              </p>
              <h3 className="mt-1 text-sm font-semibold">Citations, evidence, and activity</h3>
            </div>
            <Separator />
            <div className="min-h-0 flex-1 px-4 py-4">
              <Tabs className="flex h-full flex-col" defaultValue="citations">
                <TabsList className="w-full justify-start">
                  <TabsTrigger value="citations">Citations</TabsTrigger>
                  <TabsTrigger value="sources">Sources</TabsTrigger>
                  <TabsTrigger value="activity">Activity</TabsTrigger>
                </TabsList>
                <TabsContent className="min-h-0 flex-1" value="citations">
                  <ScrollArea className="h-full pr-2">
                    <div className="space-y-3">
                      {citationItems.map((document) => (
                        <Card key={document.id} className="rounded-2xl p-4">
                          <div className="flex items-start justify-between gap-3">
                            <div>
                              <div className="flex items-center gap-2">
                                <Badge variant="outline">{document.sourceKind}</Badge>
                                <span className="text-xs text-muted-foreground">
                                  Evidence
                                </span>
                              </div>
                              <h4 className="mt-2 text-sm font-semibold">{document.title}</h4>
                              <p className="mt-2 line-clamp-4 text-xs leading-5 text-muted-foreground">
                                {document.contentPlaintext}
                              </p>
                            </div>
                            <ArrowUpRight className="h-4 w-4 text-muted-foreground" />
                          </div>
                        </Card>
                      ))}
                    </div>
                  </ScrollArea>
                </TabsContent>
                <TabsContent className="min-h-0 flex-1" value="sources">
                  <ScrollArea className="h-full pr-2">
                    <div className="space-y-3">
                      {integrationItems.map((integration) => (
                        <Card key={integration.key} className="rounded-2xl p-4">
                          <div className="flex items-center justify-between gap-3">
                            <div>
                              <p className="text-sm font-medium">{integration.label}</p>
                              <p className="mt-1 text-xs text-muted-foreground">
                                {integration.detail ?? 'Ready for sync and retrieval'}
                              </p>
                            </div>
                            <Badge
                              variant={
                                integration.status === 'connected'
                                  ? 'success'
                                  : integration.status === 'error'
                                    ? 'destructive'
                                    : 'outline'
                              }
                            >
                              {integration.status}
                            </Badge>
                          </div>
                        </Card>
                      ))}
                    </div>
                  </ScrollArea>
                </TabsContent>
                <TabsContent className="min-h-0 flex-1" value="activity">
                  <ScrollArea className="h-full pr-2">
                    <div className="space-y-3">
                      {[
                        'Obsidian vault path is configured and ready for scanning.',
                        'Notion source can be synced into the normalized document store.',
                        'Google OAuth foundation is present for future Gmail and Calendar work.',
                      ].map((item) => (
                        <div
                          key={item}
                          className="rounded-2xl border border-border bg-secondary/60 px-4 py-3"
                        >
                          <div className="flex items-start gap-3">
                            <TimerReset className="mt-0.5 h-4 w-4 text-primary" />
                            <p className="text-sm leading-6 text-muted-foreground">{item}</p>
                          </div>
                        </div>
                      ))}
                    </div>
                  </ScrollArea>
                </TabsContent>
              </Tabs>
            </div>
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
