import {
  ArrowUpRight,
  AtSign,
  Mic,
  Paperclip,
  SearchCheck,
  Sparkles,
} from 'lucide-react';

import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';
import { Separator } from '@/components/ui/separator';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Textarea } from '@/components/ui/textarea';
import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';

const draftMessages = [
  {
    role: 'user',
    content:
      'Summarize the foundation phase and tell me which integrations are ready for grounded retrieval today.',
  },
  {
    role: 'assistant',
    content:
      'The platform can already ground answers against the normalized document store and route through the desktop backend. Notion and Obsidian are the main retrieval sources, while Google auth is ready as an integration foundation.',
  },
  {
    role: 'assistant',
    content:
      'If you want, I can turn this into an execution checklist, a risk memo, or a prioritized next sprint plan.',
  },
];

export function AssistantPage() {
  const documents = useDocumentsQuery();
  const integrations = useIntegrationSummariesQuery();

  const citationItems = (documents.data ?? []).slice(0, 5);
  const connectedSources = (integrations.data ?? []).filter(
    (item) => item.status === 'connected',
  );

  return (
    <div className="mx-auto flex h-[calc(100vh-6.25rem)] max-w-[1600px] min-h-[760px] flex-col gap-4">
      <Card className="px-6 py-5">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <div className="flex items-center gap-2">
              <Badge variant="secondary">Focused assistant</Badge>
              <Badge variant="outline">Grounded responses</Badge>
            </div>
            <h2 className="mt-3 text-2xl font-semibold tracking-tight">
              Dedicated conversation workspace with citations and source context
            </h2>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              Chat lives here now, separate from the landing overview, so responses, evidence,
              and retrieval context can stay in one focused workflow.
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Badge variant="success">
              <SearchCheck className="mr-1.5 h-3 w-3" />
              {connectedSources.length} source{connectedSources.length === 1 ? '' : 's'} connected
            </Badge>
            <Badge variant="outline">
              {citationItems.length} citation candidate{citationItems.length === 1 ? '' : 's'}
            </Badge>
          </div>
        </div>
      </Card>

      <ResizablePanelGroup
        className="min-h-0 flex-1 rounded-[30px] border border-border bg-card/65"
        direction="horizontal"
      >
        <ResizablePanel defaultSize={68} minSize={52}>
          <div className="flex h-full flex-col">
            <div className="flex items-center justify-between px-6 py-5">
              <div>
                <p className="text-xs uppercase tracking-[0.22em] text-muted-foreground">
                  Conversation
                </p>
                <h3 className="mt-2 text-lg font-semibold">Grounded response session</h3>
              </div>
              <div className="flex flex-wrap gap-2">
                <Badge variant="secondary">Streaming-ready</Badge>
                <Badge variant="outline">Source scoped</Badge>
              </div>
            </div>
            <Separator />
            <div className="flex-1 overflow-auto px-6 py-5">
              <div className="mx-auto flex max-w-4xl flex-col gap-4">
                <div className="flex flex-wrap gap-2">
                  {['Summarize latest sync state', 'Compare Notion and Obsidian notes', 'List missing integration gaps'].map(
                    (prompt) => (
                      <button
                        key={prompt}
                        className="rounded-full border border-border bg-secondary/55 px-3 py-1.5 text-xs text-muted-foreground transition hover:border-primary/30 hover:text-foreground"
                        type="button"
                      >
                        {prompt}
                      </button>
                    ),
                  )}
                </div>

                {draftMessages.map((message, index) => (
                  <div
                    key={`${message.role}-${index}`}
                    className={`flex gap-3 ${message.role === 'assistant' ? '' : 'justify-end'}`}
                  >
                    {message.role === 'assistant' ? (
                      <Avatar className="mt-1 h-9 w-9">
                        <AvatarFallback>AI</AvatarFallback>
                      </Avatar>
                    ) : null}
                    <div
                      className={`max-w-[82%] rounded-[28px] px-4 py-3 ${
                        message.role === 'assistant'
                          ? 'border border-border bg-secondary/60'
                          : 'bg-primary text-primary-foreground'
                      }`}
                    >
                      <p className="text-sm leading-7">{message.content}</p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
            <Separator />
            <div className="px-6 py-5">
              <div className="mx-auto max-w-4xl rounded-[28px] border border-border bg-background/70 p-4">
                <div className="flex flex-wrap gap-2">
                  <Badge variant="outline">@Notion</Badge>
                  <Badge variant="outline">@Obsidian</Badge>
                  <Badge variant="outline">/grounded</Badge>
                </div>
                <Textarea
                  className="mt-3 min-h-[132px] resize-none border-0 bg-transparent px-0 py-0 shadow-none focus-visible:ring-0"
                  placeholder="Ask a grounded question about your synced knowledge, source changes, or implementation risks…"
                />
                <div className="mt-4 flex items-center justify-between gap-4">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <Button size="icon" variant="ghost">
                      <Paperclip className="h-4 w-4" />
                    </Button>
                    <Button size="icon" variant="ghost">
                      <Mic className="h-4 w-4" />
                    </Button>
                    <Button size="icon" variant="ghost">
                      <AtSign className="h-4 w-4" />
                    </Button>
                    <span>Enter to send</span>
                  </div>
                  <Button className="h-11 rounded-2xl px-5">
                    Ask assistant
                    <ArrowUpRight className="ml-2 h-4 w-4" />
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </ResizablePanel>

        <ResizableHandle withHandle />

        <ResizablePanel defaultSize={32} minSize={24}>
          <div className="flex h-full flex-col">
            <div className="px-5 py-5">
              <p className="text-xs uppercase tracking-[0.22em] text-muted-foreground">
                Context
              </p>
              <h3 className="mt-2 text-lg font-semibold">Evidence, sources, and activity</h3>
            </div>
            <Separator />
            <div className="min-h-0 flex-1 px-4 py-4">
              <Tabs className="flex h-full flex-col" defaultValue="citations">
                <TabsList className="w-full justify-start">
                  <TabsTrigger value="citations">Citations</TabsTrigger>
                  <TabsTrigger value="sources">Sources</TabsTrigger>
                  <TabsTrigger value="activity">Activity</TabsTrigger>
                </TabsList>

                <TabsContent className="mt-4 flex-1 overflow-auto" value="citations">
                  <div className="space-y-3">
                    {citationItems.map((document) => (
                      <Card key={document.id} className="p-4">
                        <div className="flex items-center justify-between gap-3">
                          <div>
                            <p className="text-sm font-medium">{document.title}</p>
                            <p className="mt-1 text-xs text-muted-foreground">
                              {document.sourceKind}
                            </p>
                          </div>
                          <Badge variant="outline">Candidate</Badge>
                        </div>
                        <p className="mt-3 text-sm leading-6 text-muted-foreground">
                          {document.contentPlaintext.slice(0, 180)}...
                        </p>
                      </Card>
                    ))}
                  </div>
                </TabsContent>

                <TabsContent className="mt-4 flex-1 overflow-auto" value="sources">
                  <div className="space-y-3">
                    {connectedSources.map((source) => (
                      <Card key={source.key} className="p-4">
                        <div className="flex items-center justify-between gap-3">
                          <p className="text-sm font-medium">{source.label}</p>
                          <Badge variant="success">{source.status}</Badge>
                        </div>
                        <p className="mt-2 text-sm leading-6 text-muted-foreground">
                          {source.detail ?? 'Connected and available to the assistant.'}
                        </p>
                      </Card>
                    ))}
                  </div>
                </TabsContent>

                <TabsContent className="mt-4 flex-1 overflow-auto" value="activity">
                  <Card className="p-4">
                    <div className="flex items-center gap-2 text-sm font-medium">
                      <Sparkles className="h-4 w-4 text-primary" />
                      Streaming architecture
                    </div>
                    <div className="mt-4 space-y-3">
                      {[
                        'Prompt enters a dedicated composer with slash-style context selection.',
                        'Answer stream is reserved for grounded citations and follow-up suggestions.',
                        'Context rail keeps evidence visible without crowding the main conversation.',
                      ].map((item) => (
                        <div
                          key={item}
                          className="rounded-2xl border border-border bg-secondary/55 px-4 py-3 text-sm text-muted-foreground"
                        >
                          {item}
                        </div>
                      ))}
                    </div>
                  </Card>
                </TabsContent>
              </Tabs>
            </div>
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
