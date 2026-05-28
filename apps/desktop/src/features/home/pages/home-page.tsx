import { useMemo } from 'react';
import { Link } from 'react-router-dom';
import {
  ArrowRight,
  MessageSquare,
  Database,
  RefreshCw,
  Shield,
  Clock,
  CheckCircle2,
} from 'lucide-react';

import { useAppStatusQuery } from '@/features/dashboard/hooks/use-app-status-query';
import { useDocumentsQuery } from '@/features/documents/hooks/use-documents-query';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';

export function HomePage() {
  const appStatus = useAppStatusQuery();
  const integrations = useIntegrationSummariesQuery();
  const documents = useDocumentsQuery();

  const integrationItems = integrations.data ?? [];
  const documentItems = documents.data ?? [];
  const connectedIntegrations = integrationItems.filter((i) => i.status === 'connected');

  const retrievalReady =
    Boolean(appStatus.data?.databaseReady) &&
    Boolean(appStatus.data?.rustBackendAvailable) &&
    documentItems.length > 0;

  const recentDocs = useMemo(() => documentItems.slice(0, 3), [documentItems]);

  return (
    <div className="flex flex-col gap-6 max-w-[1480px] mx-auto animate-slide-up">
      {/* ── Top Row: Hero + System Readiness ──────────────── */}
      <div className="grid grid-cols-12 gap-5">
        {/* Hero card */}
        <section className="col-span-12 lg:col-span-8 glass-panel rounded-2xl p-8 relative overflow-hidden flex flex-col justify-center min-h-[300px]">
          <div className="hero-glow" />
          <div className="relative z-10">
            <div className="flex flex-wrap gap-2 mb-5">
              <span className="rounded-full border border-outline-variant/30 bg-surface-container-highest px-3 py-1 text-[11px] text-on-surface-variant">
                Workspace overview
              </span>
              <span className="rounded-full border border-outline-variant/20 bg-surface-container px-3 py-1 text-[11px] text-outline">
                Desktop knowledge system
              </span>
            </div>

            <h2 className="text-4xl font-bold text-on-surface max-w-2xl mb-4 tracking-tight leading-[1.15]">
              A calm, local-first workspace for grounded AI work across your knowledge sources.
            </h2>
            <p className="text-[14px] text-on-surface-variant max-w-xl mb-7 leading-relaxed">
              Keep the landing experience focused on readiness and next actions, then move into
              chat or document exploration only when you need it.
            </p>

            <div className="flex flex-wrap gap-3">
              <Link
                to="/assistant"
                id="hero-open-assistant-btn"
                className="flex items-center gap-2 rounded-xl bg-primary-container px-5 py-2.5 text-[14px] font-medium text-on-primary shadow-lg hover:brightness-110 active:scale-95 transition-all"
              >
                Open Assistant
                <ArrowRight size={16} />
              </Link>
              <Link
                to="/documents"
                id="hero-browse-knowledge-btn"
                className="glass-panel flex items-center gap-2 rounded-xl px-5 py-2.5 text-[14px] font-medium text-on-surface hover:bg-surface-container-high transition-all"
              >
                Browse Knowledge
                <ArrowRight size={16} />
              </Link>
              <Link
                to="/integrations"
                id="hero-manage-integrations-btn"
                className="flex items-center gap-2 rounded-xl border border-outline-variant/30 bg-surface-container-high/40 px-5 py-2.5 text-[14px] text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high transition-all"
              >
                Manage Integrations
                <RefreshCw size={14} />
              </Link>
            </div>
          </div>
        </section>

        {/* System Readiness panel */}
        <aside className="col-span-12 lg:col-span-4 glass-panel rounded-2xl p-6">
          <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-5">
            System Readiness
          </p>

          <div className="space-y-3">
            <div className="rounded-xl border border-surface-container-highest bg-surface-container-high p-4">
              <div className="flex items-center justify-between mb-1">
                <span className="font-medium text-on-surface text-[13px]">Retrieval pipeline</span>
                <span className={`text-[10px] font-bold uppercase rounded px-2 py-0.5 ${retrievalReady ? 'bg-tertiary/10 text-tertiary' : 'bg-surface-container-highest text-outline'}`}>
                  {retrievalReady ? 'Ready' : 'Needs setup'}
                </span>
              </div>
              <p className="text-[12px] text-on-surface-variant">
                {documentItems.length} indexed documents
              </p>
            </div>

            <div className="rounded-xl border border-surface-container-highest bg-surface-container-high p-4">
              <div className="flex items-center justify-between mb-1">
                <span className="font-medium text-on-surface text-[13px]">Source integrations</span>
                <span className="text-[10px] font-bold rounded px-2 py-0.5 bg-primary-glass/10 text-primary-glass">
                  {connectedIntegrations.length}/{integrationItems.length} Connected
                </span>
              </div>
              <p className="text-[12px] text-on-surface-variant">
                {connectedIntegrations.length > 0
                  ? connectedIntegrations.map((i) => i.label).join(', ')
                  : 'No integrations connected yet'}
              </p>
            </div>

            <div className="rounded-xl border border-surface-container-highest bg-surface-container-high p-4">
              <div className="flex items-center justify-between mb-1">
                <span className="font-medium text-on-surface text-[13px]">Desktop backend</span>
                <span className={`text-[10px] font-bold uppercase rounded px-2 py-0.5 ${appStatus.data?.rustBackendAvailable ? 'bg-tertiary/10 text-tertiary' : 'bg-surface-container-highest text-outline'}`}>
                  {appStatus.data?.rustBackendAvailable ? 'Live' : 'Pending'}
                </span>
              </div>
              <p className="text-[12px] text-on-surface-variant">
                {appStatus.data?.environment ?? 'development'}
              </p>
            </div>
          </div>
        </aside>
      </div>

      {/* ── Quick Actions ─────────────────────────────────── */}
      <section className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {[
          {
            icon: <MessageSquare size={20} className="text-primary-glass" />,
            title: 'Open Assistant',
            description: 'Start a grounded conversation with citations and source context.',
            to: '/assistant',
            id: 'qa-assistant-btn',
          },
          {
            icon: <Database size={20} className="text-primary-glass" />,
            title: 'Inspect Corpus',
            description: 'Review indexed source material in the document explorer.',
            to: '/documents',
            id: 'qa-documents-btn',
          },
          {
            icon: <RefreshCw size={20} className="text-primary-glass" />,
            title: 'Review Sync Status',
            description: 'Check integration readiness and trigger source syncs.',
            to: '/integrations',
            id: 'qa-integrations-btn',
          },
        ].map((action) => (
          <Link
            key={action.id}
            to={action.to}
            id={action.id}
            className="glass-panel rounded-2xl p-6 flex flex-col gap-4 hover:border-primary-glass/30 hover:bg-surface-container-high/30 transition-all group"
          >
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-surface-container-high border border-outline-variant/30 group-hover:border-primary-glass/30 transition-all">
              {action.icon}
            </div>
            <div>
              <p className="font-semibold text-on-surface text-[15px] group-hover:text-primary-glass transition-colors">
                {action.title}
              </p>
              <p className="mt-1 text-[13px] leading-relaxed text-on-surface-variant">
                {action.description}
              </p>
            </div>
          </Link>
        ))}
      </section>

      {/* ── Bottom Row: Research Pulse + Workspace Health ── */}
      <div className="grid grid-cols-12 gap-5">
        {/* Research Pulse */}
        <section className="col-span-12 lg:col-span-8 glass-panel rounded-2xl p-6">
          <div className="flex items-center justify-between mb-5">
            <div>
              <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-1">
                Research Pulse
              </p>
              <h3 className="text-xl font-bold text-on-surface">Recently Synthesized</h3>
            </div>
            <Link
              to="/documents"
              className="text-[12px] text-primary-glass hover:underline flex items-center gap-1"
            >
              View all projects
              <ArrowRight size={12} />
            </Link>
          </div>

          {recentDocs.length === 0 ? (
            <div className="rounded-xl border border-outline-variant/20 bg-surface-container-high/30 px-6 py-10 text-center">
              <Database size={36} className="text-outline mx-auto mb-3" />
              <p className="text-on-surface-variant text-[14px]">
                No documents indexed yet. Sync Notion or scan your Obsidian vault.
              </p>
              <Link
                to="/integrations"
                className="mt-4 inline-flex items-center gap-2 rounded-xl bg-primary-container/20 border border-primary-glass/20 px-4 py-2 text-[13px] text-primary-glass hover:bg-primary-container/30 transition-all"
              >
                Go to Integrations
                <ArrowRight size={14} />
              </Link>
            </div>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              {recentDocs.map((doc) => (
                <Link
                  key={doc.id}
                  to="/documents"
                  className="group rounded-xl border border-outline-variant/20 bg-surface-container-high/40 p-4 flex flex-col gap-3 hover:border-primary-glass/30 hover:bg-surface-container-high/60 transition-all"
                >
                  <span className={`inline-flex self-start rounded px-2 py-0.5 text-[10px] font-bold uppercase ${doc.sourceKind === 'notion' ? 'badge-notion' : 'badge-obsidian'}`}>
                    {doc.sourceKind}
                  </span>
                  <h4 className="font-bold text-on-surface text-[14px] group-hover:text-primary-glass transition-colors leading-snug">
                    {doc.title}
                  </h4>
                  <p className="text-[12px] text-on-surface-variant line-clamp-2 leading-relaxed flex-1">
                    {doc.contentPlaintext.slice(0, 120)}…
                  </p>
                  <div className="flex items-center gap-1.5 text-outline border-t border-outline-variant/20 pt-2">
                    <Clock size={11} />
                    <span className="font-mono text-[10px]">
                      {doc.updatedAt ? new Date(doc.updatedAt).toLocaleDateString() : 'Unknown'}
                    </span>
                  </div>
                </Link>
              ))}
            </div>
          )}
        </section>

        {/* Workspace Health */}
        <aside className="col-span-12 lg:col-span-4 glass-panel rounded-2xl p-6">
          <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-1">
            Workspace Health
          </p>
          <h3 className="text-lg font-bold text-on-surface mb-5">High-level readiness without dashboard noise</h3>

          <div className="space-y-4">
            <div className="flex items-start gap-3 py-4 border-b border-surface-container-highest">
              <div className="h-9 w-9 rounded-lg bg-surface-container-high flex items-center justify-center shrink-0 border border-outline-variant/30">
                <Database size={18} className="text-primary-glass" />
              </div>
              <div>
                <p className="font-mono text-[10px] uppercase tracking-wider text-outline mb-0.5">
                  Knowledge Base
                </p>
                <p className="text-2xl font-bold text-on-surface">
                  {documentItems.length}
                  <span className="text-[13px] font-normal text-on-surface-variant ml-2">
                    Normalized documents indexed locally
                  </span>
                </p>
              </div>
            </div>

            <div className="flex items-start gap-3 py-4 border-b border-surface-container-highest">
              <div className="h-9 w-9 rounded-lg bg-surface-container-high flex items-center justify-center shrink-0 border border-outline-variant/30">
                <RefreshCw size={18} className="text-primary-glass" />
              </div>
              <div>
                <p className="font-mono text-[10px] uppercase tracking-wider text-outline mb-0.5">
                  Sync Health
                </p>
                <p className="text-2xl font-bold text-on-surface">
                  {connectedIntegrations.length}
                  <span className="text-[13px] font-normal text-on-surface-variant ml-2">
                    Connected sources available for refresh
                  </span>
                </p>
              </div>
            </div>

            <div className="flex items-start gap-3 py-4">
              <div className="h-9 w-9 rounded-lg bg-surface-container-high flex items-center justify-center shrink-0 border border-outline-variant/30">
                <Shield size={18} className="text-primary-glass" />
              </div>
              <div>
                <p className="font-mono text-[10px] uppercase tracking-wider text-outline mb-0.5">
                  Offline Posture
                </p>
                <p className="text-2xl font-bold text-on-surface">
                  Stable
                  <span className="text-[13px] font-normal text-on-surface-variant ml-2">
                    Local SQLite layer available to the assistant
                  </span>
                </p>
              </div>
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}
