import { useEffect, useRef, useState } from 'react';
import {
  Paperclip,
  Mic,
  ArrowUpRight,
  CornerDownLeft,
  Zap,
} from 'lucide-react';

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
      'The platform can already ground answers against the normalized document store and route through the desktop backend. Notion and Obsidian are the main retrieval foundations currently operational.',
    streaming: true,
  },
];

export function AssistantPage() {
  const [inputText, setInputText] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
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

  return (
    <div className="flex h-[calc(100vh-8.5rem)] max-w-[1600px] mx-auto gap-6 animate-slide-up">
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
            <div className="flex gap-2">
              <span className="rounded-full border border-tertiary/20 bg-tertiary/10 px-3 py-1 text-[11px] font-medium text-tertiary">
                Streaming-ready
              </span>
              <span className="rounded-full border border-outline-variant/20 bg-surface-container-high px-3 py-1 text-[11px] text-on-surface-variant">
                Source scoped
              </span>
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
                onClick={() => setInputText(p)}
              >
                {p}
              </button>
            ))}
          </div>

          {/* Messages */}
          <div className="flex-1 overflow-y-auto custom-scrollbar space-y-5 pb-4">
            {draftMessages.map((message, index) => (
              <div
                key={`${message.role}-${index}`}
                className={`flex gap-3 items-start ${message.role === 'user' ? 'justify-end' : ''}`}
              >
                {message.role === 'assistant' && (
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-outline-variant/40 bg-surface-container-highest">
                    <Zap size={14} className="text-primary-glass" />
                  </div>
                )}
                <div
                  className={`max-w-[85%] rounded-2xl px-5 py-4 ${
                    message.role === 'user'
                      ? 'glass-panel border-primary-glass/20 bg-primary-glass/10 rounded-tr-sm'
                      : 'glass-panel rounded-tl-sm'
                  }`}
                >
                  <p
                    className={`text-[14px] leading-relaxed text-on-surface ${
                      message.streaming ? 'streaming-text' : ''
                    }`}
                  >
                    {message.content}
                  </p>
                  {message.role === 'assistant' && (
                    <div className="flex items-center gap-2 mt-3 pt-3 border-t border-outline-variant/15">
                      <span className="text-[11px] text-outline">Found in:</span>
                      {connectedSources.slice(0, 2).map((src) => (
                        <span
                          key={src.key}
                          className="rounded bg-surface-container-high px-2 py-0.5 text-[11px] text-primary-glass border border-primary-glass/20"
                        >
                          @{src.label}
                        </span>
                      ))}
                      {connectedSources.length === 0 && (
                        <>
                          <span className="rounded bg-surface-container-high px-2 py-0.5 text-[11px] text-primary-glass border border-primary-glass/20">
                            @Notion Docs
                          </span>
                          <span className="rounded bg-surface-container-high px-2 py-0.5 text-[11px] text-primary-glass border border-primary-glass/20">
                            @Obsidian Vault
                          </span>
                        </>
                      )}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>

          {/* ── Composer ─────────────────────────────────────── */}
          <div
            className={`glass-panel-solid rounded-2xl border p-4 mt-2 transition-all ${
              inputText.length > 0 ? 'animate-thinking' : 'border-outline-variant/30'
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
              className="w-full resize-none overflow-hidden border-none bg-transparent text-[14px] text-on-surface placeholder:text-outline focus:outline-none"
              placeholder="Ask a grounded question…"
              rows={1}
              id="assistant-input"
            />

            <div className="mt-2 flex items-center justify-between border-t border-outline-variant/15 pt-3">
              <div className="flex items-center gap-3 text-outline">
                <button
                  className="hover:text-primary-glass transition-colors"
                  type="button"
                  aria-label="Attach file"
                >
                  <Paperclip size={18} />
                </button>
                <button
                  className="hover:text-primary-glass transition-colors"
                  type="button"
                  aria-label="Voice input"
                >
                  <Mic size={18} />
                </button>
              </div>
              <button
                className="flex items-center gap-2 rounded-xl bg-primary-container px-5 py-2 text-[13px] font-medium text-on-primary shadow-lg hover:brightness-110 active:scale-95 transition-all"
                type="button"
                id="assistant-send-btn"
              >
                Ask assistant
                <ArrowUpRight size={15} />
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
          {['Citations', 'Sources', 'Activity'].map((tab, i) => (
            <button
              key={tab}
              className={`px-4 py-2 text-[12px] font-semibold border-b-2 transition-colors ${
                i === 0
                  ? 'border-primary-glass text-primary-glass'
                  : 'border-transparent text-outline hover:text-on-surface'
              }`}
              type="button"
            >
              {tab}
            </button>
          ))}
        </div>

        <div className="flex-1 overflow-y-auto custom-scrollbar space-y-3">
          {citationItems.length === 0 ? (
            <div className="rounded-xl border border-outline-variant/20 bg-surface-container-high/30 p-6 text-center">
              <CornerDownLeft size={28} className="text-outline mx-auto mb-2" />
              <p className="text-[13px] text-on-surface-variant">
                Citations will appear here as you ask questions.
              </p>
            </div>
          ) : (
            citationItems.map((doc) => (
              <div
                key={doc.id}
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
            ))
          )}
        </div>
      </aside>
    </div>
  );
}
