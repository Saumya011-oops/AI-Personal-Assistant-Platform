import { useEffect, useRef, useState } from 'react';
import {
  Paperclip,
  Mic,
  ArrowUpRight,
  CornerDownLeft,
  Zap,
  Loader2,
  Trash2,
  X,
} from 'lucide-react';
import type { Citation, NormalizedDocument } from '@assistant/shared';

import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';
import { invokeCommand } from '@/lib/api/invoke-command';

interface Message {
  role: 'user' | 'assistant';
  content: string;
  streaming?: boolean;
  citations?: Citation[];
  isError?: boolean;
}

const WELCOME_MESSAGE: Message = {
  role: 'assistant',
  content:
    "Hello! I am your grounded RAG assistant. Ask me any question, and I will retrieve relevant context from your connected Notion documents and Obsidian notes to provide citations alongside my response.",
};

export function AssistantPage() {
  const [inputText, setInputText] = useState('');
  const [messages, setMessages] = useState<Message[]>([WELCOME_MESSAGE]);
  const [isLoading, setIsLoading] = useState(false);
  const [activeTab, setActiveTab] = useState<'citations' | 'sources' | 'activity'>('citations');
  const [selectedDoc, setSelectedDoc] = useState<NormalizedDocument | null>(null);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  
  const documents = useDocumentsQuery();
  const integrations = useIntegrationSummariesQuery();

  const citationItems = (documents.data ?? []).slice(0, 8);
  const connectedSources = (integrations.data ?? []).filter((i) => i.status === 'connected');

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  }, [inputText]);

  // Scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSend = async (textToSend?: string) => {
    const query = (textToSend ?? inputText).trim();
    if (!query || isLoading) return;

    const userMsg: Message = { role: 'user', content: query };
    setMessages((prev) => [...prev, userMsg]);
    setInputText('');
    setIsLoading(true);

    try {
      const response = await invokeCommand('ask_assistant', { query });
      const assistantMsg: Message = {
        role: 'assistant',
        content: response.answer,
        citations: response.citations,
      };
      setMessages((prev) => [...prev, assistantMsg]);
    } catch (error: unknown) {
      console.error('Error invoking ask_assistant:', error);
      
      let errMsg = 'An unknown error occurred';
      if (error && typeof error === 'object') {
        const errRecord = error as Record<string, unknown>;
        if (typeof errRecord.message === 'string') {
          errMsg = errRecord.message;
        } else {
          errMsg = JSON.stringify(error);
        }
      } else if (typeof error === 'string') {
        errMsg = error;
      }
      
      const isApiKeyMissing = errMsg.toLowerCase().includes('groq_api_key') || 
        errMsg.toLowerCase().includes('api key') ||
        errMsg.toLowerCase().includes('not configured');

      setMessages((prev) => [
        ...prev,
        {
          role: 'assistant',
          content: isApiKeyMissing
            ? 'Error: GROQ_API_KEY is not configured. Please add your Groq API key in your `src-tauri/.env` file and restart the application.'
            : `Error: Failed to fetch response. Details: ${errMsg}`,
          isError: true,
        },
      ]);
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  // Get active citations from the latest assistant message
  const latestAssistantMessage = [...messages].reverse().find((m) => m.role === 'assistant');
  const activeCitations = latestAssistantMessage?.citations ?? [];

  // Match active citations with metadata from documents.data
  const citedDocuments = activeCitations.map((cit) => {
    const matchedDoc = (documents.data ?? []).find((doc) => doc.id === cit.documentId);
    return {
      documentId: cit.documentId,
      chunkId: cit.chunkId,
      score: cit.score,
      sourceKind: cit.source || matchedDoc?.sourceKind || 'unknown',
      title: matchedDoc?.title || `Document ${cit.documentId.slice(0, 8)}`,
      contentPlaintext: matchedDoc?.contentPlaintext || 'Context retrieved from source document.',
    };
  });

  return (
    <div className="flex h-[calc(100vh-8.5rem)] max-w-[1600px] mx-auto gap-6 animate-slide-up relative">
      {/* ── Main Chat Panel ────────────────────────────────── */}
      <section className="flex flex-1 flex-col overflow-hidden min-w-0">
        {/* Page Header */}
        <div className="pb-5">
          <div className="flex flex-wrap gap-2 mb-3">
            <span className="rounded-full border border-outline-variant/30 bg-surface-container-high px-3 py-1 text-[11px] font-medium text-on-surface">
              Focused assistant
            </span>
            <span className="rounded-full border border-outline-variant/10 bg-surface-container-low px-3 py-1 text-[11px] text-outline">
              Grounded responses
            </span>
          </div>
          <h2 className="text-2xl font-bold text-on-surface tracking-tight">
            Dedicated conversation workspace with citations and source context
          </h2>
          <p className="mt-2 text-[13px] text-on-surface-variant leading-relaxed max-w-2xl">
            Chat lives here, separate from the landing overview — responses, evidence, and
            retrieval context in one focused workflow.
          </p>
        </div>

        {/* Source + citation status pills */}
        <div className="flex flex-wrap items-center gap-3 pb-4 border-b border-surface-container-highest">
          <div className="flex items-center gap-2 rounded-full border border-primary-glass/20 bg-primary-glass/5 px-3 py-1 text-[12px] text-primary-glass">
            <span className="w-1.5 h-1.5 rounded-full bg-primary-glass animate-ai-pulse" />
            {connectedSources.length} sources connected
          </div>
          <div className="flex items-center gap-2 rounded-full border border-outline-variant/20 bg-surface-container-high px-3 py-1 text-[12px] text-on-surface-variant">
            {citationItems.length} citation candidates
          </div>
        </div>

        {/* Conversation area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Conversation label */}
          <div className="flex items-center justify-between py-4">
            <div>
              <p className="font-mono text-[10px] uppercase tracking-widest text-outline">
                Conversation
              </p>
              <h3 className="text-[15px] font-semibold text-on-surface">
                Grounded response session
              </h3>
            </div>
            <div className="flex items-center gap-2">
              <span className="rounded-full border border-tertiary/20 bg-tertiary/10 px-3 py-1 text-[11px] font-medium text-tertiary">
                Streaming-ready
              </span>
              <span className="rounded-full border border-outline-variant/20 bg-surface-container-high px-3 py-1 text-[11px] text-on-surface-variant">
                Source scoped
              </span>
              {messages.length > 1 && (
                <button
                  onClick={() => setMessages([WELCOME_MESSAGE])}
                  className="flex items-center gap-1.5 rounded-full border border-destructive/20 hover:border-destructive/50 bg-destructive/10 hover:bg-destructive/20 px-3 py-1 text-[11px] text-destructive transition-all"
                  type="button"
                  title="Clear chat history"
                >
                  <Trash2 size={12} />
                  Clear Chat
                </button>
              )}
            </div>
          </div>

          {/* Suggested prompts */}
          <div className="flex flex-wrap gap-2 pb-4">
            {[
              'Summarize latest sync state',
              'Compare Notion and Obsidian notes',
              'List missing integration gaps',
            ].map((p) => (
              <button
                key={p}
                className="rounded-xl border border-outline-variant/30 bg-surface-container-high/50 px-3 py-1.5 text-[12px] text-on-surface-variant hover:border-primary-glass/30 hover:text-on-surface transition-all"
                type="button"
                onClick={() => handleSend(p)}
                disabled={isLoading}
              >
                {p}
              </button>
            ))}
          </div>

          {/* Messages */}
          <div className="flex-1 overflow-y-auto custom-scrollbar space-y-5 pb-4">
            {messages.map((message, idx) => (
              <div
                key={`${message.role}-${idx}`}
                className={`flex gap-3 items-start ${message.role === 'user' ? 'justify-end' : ''}`}
              >
                {message.role === 'assistant' && (
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-outline-variant/40 bg-surface-container-highest">
                    <Zap size={14} className={message.isError ? 'text-destructive' : 'text-primary-glass'} />
                  </div>
                )}
                <div
                  className={`max-w-[85%] rounded-2xl px-5 py-4 ${
                    message.role === 'user'
                      ? 'glass-panel border-primary-glass/20 bg-primary-glass/10 rounded-tr-sm'
                      : message.isError
                      ? 'bg-destructive/10 border-destructive/30 border rounded-tl-sm'
                      : 'glass-panel rounded-tl-sm'
                  }`}
                >
                  <p
                    className={`text-[14px] leading-relaxed ${
                      message.isError ? 'text-destructive' : 'text-on-surface'
                    } ${message.streaming ? 'streaming-text' : ''}`}
                  >
                    {message.content}
                  </p>
                  
                  {message.role === 'assistant' && message.citations && message.citations.length > 0 && (
                    <div className="flex flex-wrap items-center gap-2 mt-3 pt-3 border-t border-outline-variant/15">
                      <span className="text-[11px] text-outline">Citations:</span>
                      {message.citations.map((src, citIdx) => (
                        <span
                          key={`${src.documentId}-${citIdx}`}
                          onClick={() => {
                            const fullDoc = (documents.data ?? []).find(d => d.id === src.documentId);
                            if (fullDoc) setSelectedDoc(fullDoc);
                          }}
                          className="rounded bg-surface-container-high hover:bg-surface-container-highest cursor-pointer px-2 py-0.5 text-[11px] text-primary-glass border border-primary-glass/20 transition-colors"
                        >
                          @{src.source || 'Doc'} [{citIdx + 1}]
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            ))}
            
            {isLoading && (
              <div className="flex gap-3 items-start">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-outline-variant/40 bg-surface-container-highest">
                  <Zap size={14} className="text-primary-glass animate-ai-pulse" />
                </div>
                <div className="glass-panel rounded-2xl rounded-tl-sm px-5 py-4 max-w-[85%] flex items-center gap-3">
                  <Loader2 size={16} className="text-primary-glass animate-spin" />
                  <span className="text-[13px] text-on-surface-variant animate-pulse">Assistant is searching and thinking...</span>
                </div>
              </div>
            )}
            
            <div ref={messagesEndRef} />
          </div>

          {/* ── Composer ─────────────────────────────────────── */}
          <div
            className={`glass-panel-solid rounded-2xl border p-4 mt-2 transition-all ${
              isLoading ? 'animate-thinking border-primary-glass/40' : 'border-outline-variant/30 focus-within:border-primary-glass/50'
            }`}
          >
            {/* Context chips */}
            <div className="flex flex-wrap gap-1.5 mb-3">
              {['@Notion', '@Obsidian', '/grounded'].map((chip) => (
                <button
                  key={chip}
                  className="rounded-lg bg-surface-container-high px-2 py-1 text-[11px] text-on-surface-variant hover:text-primary-glass transition-colors border border-outline-variant/20"
                  type="button"
                >
                  {chip}
                </button>
              ))}
            </div>

            <textarea
              ref={textareaRef}
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={handleKeyDown}
              className="w-full resize-none overflow-hidden border-none bg-transparent text-[14px] text-on-surface placeholder:text-outline focus:outline-none disabled:opacity-50"
              placeholder="Ask a grounded question…"
              rows={1}
              id="assistant-input"
              disabled={isLoading}
            />

            <div className="mt-2 flex items-center justify-between border-t border-outline-variant/15 pt-3">
              <div className="flex items-center gap-3 text-outline">
                <button
                  className="hover:text-primary-glass transition-colors disabled:opacity-50"
                  type="button"
                  aria-label="Attach file"
                  disabled={isLoading}
                >
                  <Paperclip size={18} />
                </button>
                <button
                  className="hover:text-primary-glass transition-colors disabled:opacity-50"
                  type="button"
                  aria-label="Voice input"
                  disabled={isLoading}
                >
                  <Mic size={18} />
                </button>
              </div>
              
              <button
                className={`flex items-center gap-2 rounded-xl px-5 py-2 text-[13px] font-medium shadow-lg active:scale-95 transition-all ${
                  inputText.trim() === '' || isLoading
                    ? 'bg-primary-container/40 text-on-primary/50 cursor-not-allowed'
                    : 'bg-primary-container text-on-primary hover:brightness-110'
                }`}
                type="button"
                id="assistant-send-btn"
                onClick={() => handleSend()}
                disabled={inputText.trim() === '' || isLoading}
              >
                {isLoading ? (
                  <>
                    Thinking
                    <Loader2 size={15} className="animate-spin" />
                  </>
                ) : (
                  <>
                    Ask assistant
                    <ArrowUpRight size={15} />
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      </section>

      {/* ── Right Evidence Rail ────────────────────────────── */}
      <aside className="w-[340px] shrink-0 flex flex-col border-l border-surface-container-highest bg-surface-container-low/50 -mr-6 -my-6 pl-5 pr-6 py-6">
        <div className="pb-4 border-b border-surface-container-highest">
          <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-1">
            Context
          </p>
          <h3 className="text-[18px] font-semibold text-on-surface">Evidence, sources, and activity</h3>
        </div>

        {/* Tabs: Citations / Sources / Activity */}
        <div className="flex border-b border-surface-container-highest mt-4 mb-4">
          {(['citations', 'sources', 'activity'] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={`px-4 py-2 text-[12px] font-semibold border-b-2 transition-all capitalize ${
                activeTab === tab
                  ? 'border-primary-glass text-primary-glass bg-primary-glass/5'
                  : 'border-transparent text-outline hover:text-on-surface'
              }`}
              type="button"
            >
              {tab}
            </button>
          ))}
        </div>

        {/* Tab Content */}
        {activeTab === 'citations' && (
          <div className="flex-1 overflow-y-auto custom-scrollbar space-y-3">
            {citedDocuments.length === 0 ? (
              citationItems.length > 0 ? (
                <>
                  <p className="text-[11px] text-outline font-semibold mb-1 uppercase tracking-wider">Grounded Candidates</p>
                  {citationItems.map((doc) => (
                    <div
                      key={doc.id}
                      onClick={() => setSelectedDoc(doc)}
                      className="rounded-xl border border-outline-variant/15 bg-surface-container-high/30 p-4 hover:border-primary-glass/30 transition-all cursor-pointer group"
                    >
                      <div className="flex items-start justify-between gap-2 mb-2">
                        <div className="min-w-0">
                          <h4 className="font-semibold text-on-surface text-[13px] truncate group-hover:text-primary-glass transition-colors">
                            {doc.title}
                          </h4>
                          <span
                            className={`font-mono text-[9px] uppercase font-bold ${
                              doc.sourceKind === 'notion' ? 'text-primary-glass' : 'text-tertiary'
                            }`}
                          >
                            {doc.sourceKind}
                          </span>
                        </div>
                        <span className="shrink-0 rounded bg-surface-container-highest px-1.5 py-0.5 text-[9px] text-outline border border-outline-variant/30">
                          Candidate
                        </span>
                      </div>
                      <p className="text-[12px] text-on-surface-variant leading-relaxed line-clamp-3">
                        {doc.contentPlaintext.slice(0, 160)}…
                      </p>
                    </div>
                  ))}
                </>
              ) : (
                <div className="rounded-xl border border-outline-variant/20 bg-surface-container-high/30 p-6 text-center">
                  <CornerDownLeft size={28} className="text-outline mx-auto mb-2" />
                  <p className="text-[13px] text-on-surface-variant">
                    No documents found. Connect Notion or Obsidian in the Integrations tab.
                  </p>
                </div>
              )
            ) : (
              <>
                <p className="text-[11px] text-primary-glass font-semibold mb-1 uppercase tracking-wider">Cited Sources</p>
                {citedDocuments.map((doc, idx) => (
                  <div
                    key={`${doc.documentId}-${doc.chunkId}-${idx}`}
                    onClick={() => {
                      const fullDoc = (documents.data ?? []).find(d => d.id === doc.documentId);
                      if (fullDoc) setSelectedDoc(fullDoc);
                    }}
                    className="rounded-xl border border-primary-glass/30 bg-primary-glass/5 hover:bg-primary-glass/10 p-4 hover:border-primary-glass/50 transition-all cursor-pointer group"
                  >
                    <div className="flex items-start justify-between gap-2 mb-2">
                      <div className="min-w-0">
                        <h4 className="font-semibold text-on-surface text-[13px] truncate group-hover:text-primary-glass transition-colors">
                          {doc.title}
                        </h4>
                        <span
                          className={`font-mono text-[9px] uppercase font-bold ${
                            doc.sourceKind === 'notion' ? 'text-primary-glass' : 'text-tertiary'
                          }`}
                        >
                          {doc.sourceKind}
                        </span>
                      </div>
                      <span className="shrink-0 rounded bg-primary-glass/20 px-1.5 py-0.5 text-[9px] text-primary-glass border border-primary-glass/30 font-semibold">
                        {(doc.score * 100).toFixed(0)}% Match
                      </span>
                    </div>
                    <p className="text-[12px] text-on-surface-variant leading-relaxed line-clamp-3">
                      {doc.contentPlaintext.slice(0, 160)}…
                    </p>
                  </div>
                ))}
              </>
            )}
          </div>
        )}

        {activeTab === 'sources' && (
          <div className="flex-1 overflow-y-auto custom-scrollbar space-y-3">
            <p className="text-[11px] text-outline font-semibold mb-2 uppercase tracking-wider">Connected connectors</p>
            {connectedSources.length === 0 ? (
              <div className="rounded-xl border border-outline-variant/20 bg-surface-container-high/30 p-6 text-center">
                <p className="text-[13px] text-on-surface-variant">
                  No connected sources. Please set up integrations in the sidebar menu.
                </p>
              </div>
            ) : (
              connectedSources.map((source) => (
                <div
                  key={source.key}
                  className="rounded-xl border border-outline-variant/15 bg-surface-container-high/30 p-4"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span className="w-2.5 h-2.5 rounded-full bg-emerald-500 animate-pulse" />
                      <h4 className="font-semibold text-on-surface text-[13px]">{source.label}</h4>
                    </div>
                    <span className="text-[9px] uppercase font-mono tracking-wider font-semibold text-emerald-400 bg-emerald-500/10 px-1.5 py-0.5 rounded border border-emerald-500/20">
                      active
                    </span>
                  </div>
                  {source.lastSyncedAt && (
                    <p className="text-[11px] text-outline mt-2">
                      Synced: {new Date(source.lastSyncedAt).toLocaleString()}
                    </p>
                  )}
                </div>
              ))
            )}
          </div>
        )}

        {activeTab === 'activity' && (
          <div className="flex-1 overflow-y-auto custom-scrollbar space-y-4">
            <p className="text-[11px] text-outline font-semibold mb-1 uppercase tracking-wider">Session Event Log</p>
            <div className="relative border-l border-outline-variant/30 pl-4 space-y-4 ml-2 mt-2">
              <div className="relative">
                <span className="absolute -left-[21px] top-1 w-2.5 h-2.5 rounded-full bg-emerald-500 border border-surface-container-low" />
                <p className="text-[12px] font-semibold text-on-surface">Assistant Workspace Ready</p>
                <p className="text-[10px] text-outline">Listening on local environment</p>
              </div>
              {messages.map((m, idx) => {
                if (idx === 0) return null; // skip default welcome message
                return (
                  <div key={idx} className="relative">
                    <span className={`absolute -left-[21px] top-1 w-2.5 h-2.5 rounded-full border border-surface-container-low ${
                      m.role === 'user' ? 'bg-primary-glass' : m.isError ? 'bg-destructive' : 'bg-tertiary'
                    }`} />
                    <p className="text-[12px] font-semibold text-on-surface capitalize">
                      {m.role === 'user' ? 'Grounded query sent' : m.isError ? 'Error occurred' : 'Assistant response received'}
                    </p>
                    <p className="text-[11px] text-on-surface-variant line-clamp-1">
                      {m.content}
                    </p>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </aside>

      {/* ── Document Details Slide-over Drawer ──────────────── */}
      {selectedDoc && (
        <div
          onClick={() => setSelectedDoc(null)}
          className="fixed inset-0 z-50 flex items-center justify-end bg-black/60 backdrop-blur-sm transition-all duration-300"
        >
          <div
            onClick={(e) => e.stopPropagation()}
            className="w-[600px] h-full bg-surface-container-low border-l border-outline-variant/30 p-6 flex flex-col justify-between shadow-2xl animate-slide-up"
          >
            {/* Header */}
            <div className="flex items-start justify-between pb-4 border-b border-outline-variant/20">
              <div className="min-w-0">
                <span className={`inline-block px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider mb-2 ${
                  selectedDoc.sourceKind === 'notion' ? 'badge-notion' : 'badge-obsidian'
                }`}>
                  {selectedDoc.sourceKind}
                </span>
                <h3 className="text-lg font-bold text-on-surface leading-snug truncate">
                  {selectedDoc.title}
                </h3>
                {selectedDoc.pathOrUrl && (
                  <p className="text-xs text-outline mt-1 font-mono break-all truncate">
                    {selectedDoc.pathOrUrl}
                  </p>
                )}
              </div>
              <button
                onClick={() => setSelectedDoc(null)}
                className="p-1.5 hover:bg-surface-container-highest rounded-full text-outline hover:text-on-surface transition-colors shrink-0 ml-4"
                type="button"
                aria-label="Close details"
              >
                <X size={20} />
              </button>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto custom-scrollbar py-6 space-y-6">
              <div>
                <h4 className="text-xs font-semibold text-outline uppercase tracking-wider mb-2">Content</h4>
                <div className="prose prose-invert max-w-none text-sm text-on-surface-variant leading-relaxed whitespace-pre-wrap font-sans bg-surface-container-high/40 p-4 rounded-xl border border-outline-variant/15">
                  {selectedDoc.contentMarkdown || selectedDoc.contentPlaintext}
                </div>
              </div>

              {selectedDoc.tags && selectedDoc.tags.length > 0 && (
                <div>
                  <h4 className="text-xs font-semibold text-outline uppercase tracking-wider mb-2">Tags</h4>
                  <div className="flex flex-wrap gap-1.5">
                    {selectedDoc.tags.map((tag) => (
                      <span
                        key={tag}
                        className="px-2 py-0.5 rounded bg-surface-container-highest text-xs text-on-surface border border-outline-variant/20"
                      >
                        #{tag}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {selectedDoc.metadata && Object.keys(selectedDoc.metadata).length > 0 && (
                <div>
                  <h4 className="text-xs font-semibold text-outline uppercase tracking-wider mb-2">Metadata</h4>
                  <pre className="text-xs font-mono bg-surface-container-highest/50 p-4 rounded-xl overflow-x-auto text-on-surface-variant border border-outline-variant/15">
                    {JSON.stringify(selectedDoc.metadata, null, 2)}
                  </pre>
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="pt-4 border-t border-outline-variant/20 flex justify-end">
              <button
                onClick={() => setSelectedDoc(null)}
                className="px-5 py-2 rounded-xl bg-surface-container-highest hover:bg-outline-variant/20 text-sm font-medium text-on-surface transition-all"
                type="button"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
