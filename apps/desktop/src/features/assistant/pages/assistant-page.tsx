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
  Plus,
  Search,
  MessageSquare,
  Edit,
  Brain,
  Download,
  Upload,
  Check,
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
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
  diagnostics?: any;
  memories?: any[];
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
  const [activeTab, setActiveTab] = useState<'citations' | 'sources' | 'activity' | 'memories' | 'conversation'>('citations');
  const [selectedDoc, setSelectedDoc] = useState<NormalizedDocument | null>(null);

  // Chat History & Memories State
  const [chats, setChats] = useState<any[]>([]);
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);
  const [chatSearchQuery, setChatSearchQuery] = useState('');
  const [renamingChatId, setRenamingChatId] = useState<string | null>(null);
  const [renameText, setRenameText] = useState('');
  const [conversationSummary, setConversationSummary] = useState('');
  const [memoryReady, setMemoryReady] = useState(false);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  
  const documents = useDocumentsQuery();
  const integrations = useIntegrationSummariesQuery();

  const citationItems = (documents.data ?? []).slice(0, 8);
  const connectedSources = (integrations.data ?? []).filter((i) => i.status === 'connected');

  const loadChats = async () => {
    try {
      const chatList = await invokeCommand('list_chats', {});
      setChats(chatList || []);
    } catch (e) {
      console.error('Failed to load chats:', e);
    }
  };

  const checkMemoryStatus = async () => {
    try {
      // RAG and Memory collection status check
      await invokeCommand('list_memories', {});
      setMemoryReady(true);
    } catch (e) {
      setMemoryReady(false);
    }
  };

  useEffect(() => {
    loadChats();
    checkMemoryStatus();
  }, []);

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

  const handleSelectChat = async (id: string) => {
    try {
      setActiveConversationId(id);
      setRenamingChatId(null);
      
      const msgsResp = await invokeCommand('load_chat_messages', { conversationId: id });
      const dbMsgs = msgsResp || [];
      const formattedMsgs: Message[] = dbMsgs.map((m: any) => {
        return {
          role: m.role as 'user' | 'assistant',
          content: m.content,
          citations: (() => {
            if (typeof m.citations !== 'string' || !m.citations.trim()) return undefined;
            try {
              return JSON.parse(m.citations);
            } catch {
              return undefined;
            }
          })(),
        };
      });
      
      if (formattedMsgs.length === 0) {
        setMessages([WELCOME_MESSAGE]);
      } else {
        setMessages(formattedMsgs);
      }

      // Fetch summary defensively
      try {
        const summaryResp = await invokeCommand('get_conversation_summary', { conversationId: id });
        setConversationSummary(summaryResp || '');
      } catch (summaryError) {
        console.error('Failed to load conversation summary:', summaryError);
        setConversationSummary('');
      }
    } catch (e) {
      console.error('Failed to load chat history:', e);
    }
  };

  const handleNewChat = () => {
    setActiveConversationId(null);
    setMessages([WELCOME_MESSAGE]);
    setConversationSummary('');
    setRenamingChatId(null);
  };

  const handleDeleteChat = async (id: string) => {
    if (!window.confirm("Are you sure you want to delete this conversation?")) return;
    try {
      await invokeCommand('delete_chat', { id });
      loadChats();
      if (activeConversationId === id) {
        handleNewChat();
      }
    } catch (e) {
      console.error('Failed to delete chat:', e);
    }
  };

  const handleSaveRename = async (id: string) => {
    if (!renameText.trim()) return;
    try {
      await invokeCommand('rename_chat', { id, title: renameText });
      setRenamingChatId(null);
      loadChats();
    } catch (e) {
      console.error('Failed to rename chat:', e);
    }
  };

  const getGroupedChats = () => {
    const grouped = {
      Today: [] as any[],
      Yesterday: [] as any[],
      'Last Week': [] as any[],
      Older: [] as any[],
    };

    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);
    const lastWeek = new Date(today);
    lastWeek.setDate(lastWeek.getDate() - 7);

    const filtered = chats.filter((c) =>
      (c.title || '').toLowerCase().includes(chatSearchQuery.toLowerCase()) ||
      (c.summary || '').toLowerCase().includes(chatSearchQuery.toLowerCase())
    );

    filtered.forEach((chat) => {
      if (!chat.updated_at) return;
      const chatDate = new Date(chat.updated_at.replace(' ', 'T') + 'Z');
      if (chatDate >= today) {
        grouped.Today.push(chat);
      } else if (chatDate >= yesterday) {
        grouped.Yesterday.push(chat);
      } else if (chatDate >= lastWeek) {
        grouped['Last Week'].push(chat);
      } else {
        grouped.Older.push(chat);
      }
    });

    return grouped;
  };

  const handleSend = async (textToSend?: string) => {
    const query = (textToSend ?? inputText).trim();
    if (!query || isLoading) return;

    const userMsg: Message = { role: 'user', content: query };
    setMessages((prev) => [...prev, userMsg]);
    setInputText('');
    setIsLoading(true);

    try {
      const response: any = await invokeCommand('ask_assistant', {
        query,
        conversationId: activeConversationId || undefined,
      });
      const assistantMsg: Message = {
        role: 'assistant',
        content: response.answer,
        citations: response.citations,
        diagnostics: response.diagnostics,
        memories: response.memories,
      };
      setMessages((prev) => [...prev, assistantMsg]);

      if (response.conversationId) {
        setActiveConversationId(response.conversationId);
        loadChats();
        
        // Fetch summary defensively
        try {
          const summaryResp = await invokeCommand('get_conversation_summary', {
            conversationId: response.conversationId,
          });
          setConversationSummary(summaryResp || '');
        } catch (summaryError) {
          console.error('Failed to load conversation summary:', summaryError);
        }
      }
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
  const citedDocuments = activeCitations.map((cit, idx) => {
    const matchedDoc = (documents.data ?? []).find((doc) => doc.id === cit.documentId);
    const resolvedTitle =
      (cit as any).documentTitle ||
      (cit as any).sourceDocument ||
      matchedDoc?.title ||
      `Document ${(cit.documentId ?? '').slice(0, 8)}`;
    // [CITATION_UI] diagnostic: shows exactly what the frontend receives at runtime
    console.log(
      `[CITATION_UI] id=${cit.documentId}`,
      `documentTitle=${(cit as any).documentTitle}`,
      `sourceDocument=${(cit as any).sourceDocument}`,
      `matchedDocTitle=${matchedDoc?.title}`,
      `rendered="${resolvedTitle}"`,
      '| full_object:', JSON.stringify(cit),
    );
    return {
      documentId: cit.documentId,
      chunkId: cit.chunkId,
      title: resolvedTitle,
      rerankScore: (cit as any).rerankScore ?? cit.score ?? 0,
      sourceKind: (cit as any).sourceConnector || (cit as any).sourceType || cit.source || matchedDoc?.sourceKind || 'unknown',
      contentPlaintext: matchedDoc?.contentPlaintext || 'Context retrieved from source document.',
      section: (cit as any).section || 'General',
      evidence: (cit as any).evidenceSnippet || (cit as any).evidence,
      evidenceLevel: (cit as any).evidenceLevel || 'Supporting Evidence',
      idx,
    };
  });

  const groupedChats = getGroupedChats();
  const activeChat = chats.find((c) => c.id === activeConversationId);
  const activeChatTitle = activeChat ? activeChat.title : 'New Chat';

  return (
    <div className="flex h-[calc(100vh-8.5rem)] max-w-[1600px] mx-auto gap-6 animate-slide-up relative">
      {/* ── Left Sidebar: Chat History ──────────────────────── */}
      <aside className="w-[260px] shrink-0 flex flex-col border-r border-surface-container-highest/30 bg-surface-container-low/10 -ml-6 -my-6 py-6 px-4">
        {/* New Chat Button */}
        <button
          onClick={handleNewChat}
          className="flex items-center justify-center gap-2 rounded-xl bg-primary-glass px-4 py-3 text-[13px] font-bold text-black shadow-lg hover:glow active:scale-95 transition-all mb-4"
        >
          <Plus size={16} />
          New Chat
        </button>

        {/* Search Input */}
        <div className="flex items-center gap-2 rounded-xl border border-outline-variant/20 bg-[#0b1326]/20 px-3 py-2 mb-4">
          <Search size={14} className="text-outline shrink-0" />
          <input
            type="text"
            placeholder="Search chats..."
            value={chatSearchQuery}
            onChange={(e) => setChatSearchQuery(e.target.value)}
            className="flex-1 bg-transparent text-[12px] text-on-surface focus:outline-none placeholder:text-outline min-w-0"
          />
        </div>

        {/* Chats List grouped by date */}
        <div className="flex-1 overflow-y-auto custom-scrollbar space-y-4 pr-1">
          {Object.entries(groupedChats).map(([group, groupChats]) => {
            if ((groupChats as any[]).length === 0) return null;
            return (
              <div key={group} className="space-y-1">
                <p className="font-mono text-[9px] font-bold uppercase tracking-widest text-outline px-2 mb-1">
                  {group}
                </p>
                <div className="space-y-0.5">
                  {(groupChats as any[]).map((chat) => {
                    const isActive = chat.id === activeConversationId;
                    const isRenaming = renamingChatId === chat.id;
                    return (
                      <div
                        key={chat.id}
                        onClick={() => !isRenaming && handleSelectChat(chat.id)}
                        className={`group relative flex items-center justify-between rounded-xl px-3 py-2.5 cursor-pointer transition-all border border-transparent ${
                          isActive
                            ? 'bg-primary-glass/10 border-primary-glass/20 text-primary-glass font-medium'
                            : 'hover:bg-surface-container-high/40 text-on-surface-variant hover:text-on-surface'
                        }`}
                      >
                        <div className="flex items-center gap-2 min-w-0 flex-1">
                          <MessageSquare size={14} className="shrink-0 text-outline group-hover:text-on-surface" />
                          {isRenaming ? (
                            <input
                              type="text"
                              value={renameText}
                              onChange={(e) => setRenameText(e.target.value)}
                              onKeyDown={(e) => {
                                if (e.key === 'Enter') handleSaveRename(chat.id);
                                if (e.key === 'Escape') setRenamingChatId(null);
                              }}
                              onClick={(e) => e.stopPropagation()}
                              autoFocus
                              className="flex-1 bg-[#0b1326]/60 border border-primary-glass/50 rounded px-1.5 py-0.5 text-[12px] text-on-surface focus:outline-none"
                            />
                          ) : (
                            <span className="text-[12px] truncate font-light leading-snug">
                              {chat.title || 'Untitled Conversation'}
                            </span>
                          )}
                        </div>

                        {/* Hover actions */}
                        {!isRenaming && (
                          <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity ml-1.5 shrink-0">
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                setRenamingChatId(chat.id);
                                setRenameText(chat.title || '');
                              }}
                              className="p-1 hover:bg-surface-container-highest rounded text-outline hover:text-on-surface transition-colors"
                              title="Rename chat"
                            >
                              <Edit size={11} />
                            </button>
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                handleDeleteChat(chat.id);
                              }}
                              className="p-1 hover:bg-destructive/15 rounded text-outline hover:text-destructive transition-colors"
                              title="Delete chat"
                            >
                              <Trash2 size={11} />
                            </button>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      </aside>

      {/* ── Main Chat Panel ────────────────────────────────── */}
      <section className="flex flex-1 flex-col overflow-hidden min-w-0">


        {/* Conversation area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Conversation label */}
          <div className="flex items-center justify-between py-4">
            <div>
              <p className="font-mono text-[10px] uppercase tracking-widest text-outline">
                Conversation
              </p>
              <h3 className="text-[15px] font-semibold text-on-surface">
                {activeChatTitle}
              </h3>
            </div>
            <div className="flex items-center gap-2">
              {memoryReady && (
                <span className="rounded-full border border-emerald-500/25 bg-emerald-500/10 px-3 py-1 text-[11px] font-medium text-emerald-400">
                  Memory Ready
                </span>
              )}
              <span className="rounded-full border border-emerald-500/25 bg-emerald-500/10 px-3 py-1 text-[11px] font-medium text-emerald-400">
                Retrieval Ready
              </span>
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
            <AnimatePresence initial={false}>
              {messages.map((message, idx) => (
                <motion.div
                  key={`${message.role}-${idx}`}
                  initial={{ opacity: 0, y: 12 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ type: 'spring', stiffness: 350, damping: 28 }}
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
                      <div className="flex flex-col gap-2 mt-4 pt-3 border-t border-outline-variant/15">
                        <span className="text-[11px] text-outline font-semibold uppercase tracking-wider mb-1">Evidence Sources</span>
                        {message.citations.map((src, citIdx) => {
                          // Look up document metadata by ID — same fallback the right panel uses
                          const matchedDoc = (documents.data ?? []).find(d => d.id === src.documentId);
                          const sdoc =
                            (src as any).documentTitle ||
                            (src as any).sourceDocument ||
                            matchedDoc?.title ||
                            (src.documentId ? `Document ${src.documentId.slice(0, 8)}` : 'Unknown Source');
                          // [CITATION_UI] diagnostic for inline card render
                          console.log(
                            `[CITATION_UI][card] id=${src.documentId}`,
                            `documentTitle=${(src as any).documentTitle}`,
                            `sourceDocument=${(src as any).sourceDocument}`,
                            `matchedDocTitle=${matchedDoc?.title}`,
                            `rendered="${sdoc}"`,
                            '| full_object:', JSON.stringify(src),
                          );
                          const sconn = (src as any).sourceConnector || src.source || 'doc';
                          const stype = sconn.toLowerCase();
                          const section = (src as any).section || 'General';
                          const evidence = (src as any).evidenceSnippet || (src as any).evidence;
                          const evidenceLevel = (src as any).evidenceLevel || 'Supporting Evidence';

                          let pillStyle = '';
                          if (evidenceLevel === 'High Evidence') {
                            pillStyle = 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20';
                          } else if (evidenceLevel === 'Medium Evidence') {
                            pillStyle = 'bg-amber-500/10 text-amber-400 border border-amber-500/20';
                          } else {
                            pillStyle = 'bg-outline-variant/10 text-outline border border-outline-variant/20';
                          }

                          const icon = stype.includes('notion') ? '🟣' : stype.includes('obsidian') ? '📁' : '📄';

                          return (
                            <div
                              key={`${src.documentId}-${citIdx}`}
                              onClick={() => {
                                const fullDoc = (documents.data ?? []).find(d => d.id === src.documentId);
                                if (fullDoc) setSelectedDoc(fullDoc);
                              }}
                              className="rounded-xl bg-surface-container-high/60 hover:bg-surface-container-highest cursor-pointer p-3.5 border border-primary-glass/10 hover:border-primary-glass/30 transition-all flex flex-col gap-2 shadow-sm group"
                            >
                              <div className="flex items-center justify-between gap-2">
                                <div className="flex items-center gap-2 min-w-0 flex-1">
                                  <span className="text-[14px]">{icon}</span>
                                  <div className="flex flex-col min-w-0">
                                    <p className="text-[12px] font-bold text-on-surface truncate group-hover:text-primary-glass transition-colors">{sdoc}</p>
                                    <p className="text-[9px] text-outline font-semibold uppercase tracking-wider">{sconn}</p>
                                  </div>
                                </div>
                                <span className={`text-[9px] px-2 py-0.5 rounded-full font-bold shrink-0 border uppercase tracking-wider ${pillStyle}`}>
                                  {evidenceLevel}
                                </span>
                              </div>

                              <div className="text-[11px] text-on-surface-variant flex flex-col gap-1 pl-5">
                                <p className="font-semibold text-outline-variant">
                                  <span className="text-outline">Section:</span> {section}
                                </p>
                                {evidence && (
                                  <p className="italic text-on-surface-variant/80 border-l-2 border-primary-glass/30 pl-2 py-0.5 mt-0.5">
                                    "{evidence}"
                                  </p>
                                )}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                    {message.role === 'assistant' && message.diagnostics && (
                      <details className="mt-4 text-[11px] text-on-surface-variant bg-surface-container-high/30 rounded-xl border border-outline-variant/15 p-3.5 cursor-pointer select-none group transition-all">
                        <summary className="font-semibold text-outline hover:text-primary-glass transition-colors list-none flex items-center gap-1">
                          <span className="group-open:rotate-90 transition-transform duration-200">▶</span>
                          Developer Diagnostics
                        </summary>
                        <div className="mt-3 grid grid-cols-2 gap-4 border-t border-outline-variant/15 pt-3 select-text cursor-default">
                          <div className="space-y-1.5 font-mono">
                            <p><span className="text-outline">Strategy:</span> {message.diagnostics.strategy}</p>
                            <p><span className="text-outline">Intent:</span> {message.diagnostics.finalStatus}</p>
                            <p><span className="text-outline">Confidence:</span> {message.diagnostics.confidenceBreakdown?.finalScore ?? 0}/100</p>
                          </div>
                          <div className="space-y-1.5 font-mono">
                            <p><span className="text-outline">Recall unique docs pre:</span> {message.diagnostics.recallMetrics?.uniqueDocsPreRerank ?? 0}</p>
                            <p><span className="text-outline">Recall unique docs post:</span> {message.diagnostics.recallMetrics?.uniqueDocsPostRerank ?? 0}</p>
                            <p><span className="text-outline">Top doc changed:</span> {message.diagnostics.recallMetrics?.topDocChanged ? 'Yes' : 'No'}</p>
                          </div>
                          
                          <div className="col-span-2 border-t border-outline-variant/15 pt-2 mt-1">
                            <p className="font-bold text-outline uppercase tracking-wider text-[9px] mb-1.5">Pre-Rerank Chunks</p>
                            <div className="space-y-1 font-mono text-[10px] max-h-24 overflow-y-auto custom-scrollbar">
                              {message.diagnostics.preRerankChunks?.map((chunk: any, i: number) => (
                                <p key={i} className="truncate">
                                  {i + 1}. {chunk.documentTitle} (retrieval: {chunk.retrievalScore?.toFixed(3) ?? '0'})
                                </p>
                              )) ?? <p className="text-outline italic">No pre-rerank chunks</p>}
                            </div>
                          </div>

                          <div className="col-span-2 border-t border-outline-variant/15 pt-2">
                            <p className="font-bold text-outline uppercase tracking-wider text-[9px] mb-1.5">Post-Rerank Chunks</p>
                            <div className="space-y-1 font-mono text-[10px] max-h-24 overflow-y-auto custom-scrollbar">
                              {message.diagnostics.postRerankChunks?.map((chunk: any, i: number) => (
                                <p key={i} className="truncate">
                                  {i + 1}. {chunk.documentTitle} (rerank logit: {chunk.rerankScore?.toFixed(3) ?? '0'})
                                </p>
                              )) ?? <p className="text-outline italic">No post-rerank chunks</p>}
                            </div>
                          </div>
                        </div>
                      </details>
                    )}
                  </div>
                </motion.div>
              ))}
              
              {isLoading && (
                <motion.div
                  initial={{ opacity: 0, y: 12 }}
                  animate={{ opacity: 1, y: 0 }}
                  className="flex gap-3 items-start"
                >
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-outline-variant/40 bg-surface-container-highest">
                    <Zap size={14} className="text-primary-glass animate-ai-pulse" />
                  </div>
                  <div className="glass-panel rounded-2xl rounded-tl-sm px-5 py-4 max-w-[85%] flex items-center gap-3">
                    <Loader2 size={16} className="text-primary-glass animate-spin" />
                    <span className="text-[13px] text-on-surface-variant animate-pulse">Assistant is searching and thinking...</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            
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

        {/* Tabs: Citations / Sources / Activity / Memories / Conversation */}
        <div className="flex border-b border-surface-container-highest/40 mt-4 mb-4 relative overflow-x-auto no-scrollbar">
          {(['citations', 'sources', 'activity', 'memories', 'conversation'] as const).map((tab) => {
            const isActive = activeTab === tab;
            let displayLabel: string = tab;
            if (tab === 'activity') displayLabel = 'log';
            if (tab === 'conversation') displayLabel = 'summary';
            return (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                className={`relative px-4 py-2.5 text-[12px] font-semibold transition-colors capitalize ${
                  isActive
                    ? 'text-primary-glass'
                    : 'text-outline hover:text-on-surface'
                }`}
                type="button"
              >
                {isActive && (
                  <motion.div
                    layoutId="active-assistant-tab"
                    className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary-glass"
                    transition={{ type: 'spring', stiffness: 380, damping: 30 }}
                  />
                )}
                <span className="relative z-10">{displayLabel}</span>
              </button>
            );
          })}
        </div>

        {/* Tab Content */}
        <div className="flex-1 overflow-y-auto custom-scrollbar">
          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.15 }}
              className="space-y-3 min-h-full"
            >
              {activeTab === 'citations' && (
                <div className="space-y-3">
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
                      {citedDocuments.map((doc) => {
                        const icon = doc.sourceKind.toLowerCase() === 'notion' ? '🟣' : doc.sourceKind.toLowerCase() === 'obsidian' ? '📁' : '📄';

                        let pillStyle = '';
                        if (doc.evidenceLevel === 'High Evidence') {
                          pillStyle = 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20';
                        } else if (doc.evidenceLevel === 'Medium Evidence') {
                          pillStyle = 'bg-amber-500/10 text-amber-400 border border-amber-500/20';
                        } else {
                          pillStyle = 'bg-outline-variant/10 text-outline border border-outline-variant/20';
                        }

                        return (
                          <div
                            key={`${doc.documentId}-${doc.chunkId}-${doc.idx}`}
                            onClick={() => {
                              const fullDoc = (documents.data ?? []).find(d => d.id === doc.documentId);
                              if (fullDoc) setSelectedDoc(fullDoc);
                            }}
                            className="rounded-xl border border-primary-glass/20 bg-surface-container-high/40 hover:bg-surface-container-highest p-4 hover:border-primary-glass/40 transition-all cursor-pointer flex flex-col gap-2 group"
                          >
                            <div className="flex items-start justify-between gap-2">
                              <div className="min-w-0 flex-1">
                                <div className="flex items-center gap-1.5">
                                  <span className="text-[12px]">{icon}</span>
                                  <h4 className="font-bold text-on-surface text-[13px] truncate group-hover:text-primary-glass transition-colors">
                                    {doc.title}
                                  </h4>
                                </div>
                                <div className="flex items-center gap-2 mt-1 pl-5">
                                  <span className={`font-mono text-[9px] uppercase font-bold ${
                                    doc.sourceKind.toLowerCase() === 'notion' ? 'text-primary-glass' : 'text-tertiary'
                                  }`}>
                                    {doc.sourceKind}
                                  </span>
                                  <span className="text-[10px] text-outline-variant">|</span>
                                  <span className="text-[10px] text-outline">Section: {doc.section}</span>
                                </div>
                              </div>
                              <span className={`shrink-0 rounded-full px-2 py-0.5 text-[9px] font-bold border uppercase tracking-wider ${pillStyle}`}>
                                {doc.evidenceLevel}
                              </span>
                            </div>

                            {doc.evidence && (
                              <p className="text-[11px] italic text-on-surface-variant/80 border-l border-primary-glass/30 pl-2 ml-5 py-0.5">
                                "{doc.evidence}"
                              </p>
                            )}

                            <p className="text-[11px] text-on-surface-variant leading-relaxed line-clamp-3 ml-5 font-light">
                              {doc.contentPlaintext.slice(0, 160)}…
                            </p>
                          </div>
                        );
                      })}
                    </>
                  )}
                </div>
              )}

              {activeTab === 'sources' && (
                <div className="space-y-3">
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
                <div className="space-y-4">
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

              {activeTab === 'memories' && (
                <div className="space-y-3">
                  <p className="text-[11px] text-outline font-semibold mb-2 uppercase tracking-wider">Used Memories (Latest Turn)</p>
                  {(() => {
                    const lastAssistant = [...messages].reverse().find(m => m.role === 'assistant');
                    const usedMemories = lastAssistant?.memories || [];
                    if (usedMemories.length === 0) {
                      return (
                        <div className="rounded-xl border border-outline-variant/20 bg-surface-container-high/30 p-6 text-center text-on-surface-variant text-[13px]">
                          No memories retrieved in the latest conversation turn.
                        </div>
                      );
                    }
                    return usedMemories.map((m: any, index: number) => (
                      <div
                        key={m.id || index}
                        className="rounded-xl border border-outline-variant/15 bg-surface-container-high/30 p-4 space-y-2"
                      >
                        <p className="text-[13px] text-on-surface leading-relaxed font-light">{m.content}</p>
                        <div className="flex flex-wrap items-center gap-2 text-[10px] text-outline">
                          <span className="font-bold text-primary-glass bg-primary-glass/10 px-1.5 py-0.5 rounded border border-primary-glass/25 uppercase text-[8px] tracking-wider">
                            {m.type}
                          </span>
                          <span>Score: <strong>{m.finalScore ? (m.finalScore * 100).toFixed(0) : (m.similarity * 100).toFixed(0)}%</strong></span>
                          <span>•</span>
                          <span>Importance: <strong>{m.importanceScore || m.importance}/10</strong></span>
                        </div>
                        {m.last_used && (
                          <p className="text-[9px] text-outline">Last Used: {new Date(m.last_used.replace(' ', 'T') + 'Z').toLocaleString()}</p>
                        )}
                      </div>
                    ));
                  })()}
                </div>
              )}

              {activeTab === 'conversation' && (
                <div className="space-y-3">
                  <p className="text-[11px] text-outline font-semibold mb-2 uppercase tracking-wider">Active Chat Summary</p>
                  {conversationSummary ? (
                    <div className="rounded-xl border border-outline-variant/15 bg-surface-container-high/30 p-4 text-[13px] text-on-surface-variant leading-relaxed font-light whitespace-pre-wrap">
                      {conversationSummary}
                    </div>
                  ) : (
                    <div className="rounded-xl border border-outline-variant/20 bg-surface-container-high/30 p-6 text-center text-on-surface-variant text-[13px]">
                      No summary available yet. Summaries are auto-generated as the chat history grows.
                    </div>
                  )}
                </div>
              )}
            </motion.div>
          </AnimatePresence>
        </div>
      </aside>

      {/* ── Document Details Slide-over Drawer ──────────────── */}
      <AnimatePresence>
        {selectedDoc && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => setSelectedDoc(null)}
            className="fixed inset-0 z-50 flex items-center justify-end bg-black/60 backdrop-blur-sm"
          >
            <motion.div
              initial={{ x: '100%' }}
              animate={{ x: 0 }}
              exit={{ x: '100%' }}
              transition={{ type: 'spring', stiffness: 320, damping: 28 }}
              onClick={(e) => e.stopPropagation()}
              className="w-[600px] h-full bg-[#0b1326]/90 backdrop-blur-xl border-l border-outline-variant/20 p-6 flex flex-col justify-between shadow-2xl"
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
                  <div className="prose prose-invert max-w-none text-sm text-on-surface-variant leading-relaxed whitespace-pre-wrap font-sans bg-[#0e192f]/50 p-4 rounded-xl border border-outline-variant/15">
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
                          className="px-2 py-0.5 rounded bg-[#0e192f]/50 text-xs text-on-surface border border-outline-variant/20"
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
                    <pre className="text-xs font-mono bg-[#0e192f]/30 p-4 rounded-xl overflow-x-auto text-on-surface-variant border border-outline-variant/15">
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
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
