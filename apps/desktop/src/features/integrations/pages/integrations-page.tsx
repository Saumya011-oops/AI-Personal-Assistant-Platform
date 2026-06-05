import { Loader2, RefreshCw, CheckCircle2, FolderOpen, Mail, Calendar, Plus } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

import { ErrorState } from '@/components/states/error-state';
import { LoadingState } from '@/components/states/loading-state';
import { useGoogleConnectMutation } from '@/features/integrations/hooks/use-google-connect-mutation';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';
import { useNotionSyncMutation } from '@/features/integrations/hooks/use-notion-sync-mutation';
import { useObsidianScanMutation } from '@/features/integrations/hooks/use-obsidian-scan-mutation';

const containerVariants = {
  hidden: { opacity: 0 },
  show: {
    opacity: 1,
    transition: {
      staggerChildren: 0.04,
    },
  },
};

const itemVariants = {
  hidden: { opacity: 0, y: 12 },
  show: {
    opacity: 1,
    y: 0,
    transition: {
      type: 'spring' as const,
      stiffness: 280,
      damping: 24,
    },
  },
};

// ── Icon map for each integration ──────────────────────────
function IntegrationIcon({ kind }: { kind: string }) {
  const cls = 'text-primary-glass';
  switch (kind) {
    case 'notion':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={`h-6 w-6 ${cls}`} stroke="currentColor" strokeWidth={1.5}>
          <rect x="3" y="3" width="18" height="18" rx="3" />
          <path d="M7 8h10M7 12h7M7 16h5" strokeLinecap="round" />
        </svg>
      );
    case 'obsidian':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={`h-6 w-6 ${cls}`} stroke="currentColor" strokeWidth={1.5}>
          <polygon points="12,2 22,8 22,16 12,22 2,16 2,8" />
          <line x1="12" y1="2" x2="12" y2="22" />
          <line x1="2" y1="8" x2="22" y2="16" />
          <line x1="22" y1="8" x2="2" y2="16" />
        </svg>
      );
    case 'google':
      return (
        <svg viewBox="0 0 24 24" className={`h-6 w-6 ${cls}`} fill="currentColor">
          <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" />
          <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" />
          <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" />
          <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" />
        </svg>
      );
    default:
      return <RefreshCw size={22} className={cls} />;
  }
}

function statusConfig(status: string) {
  switch (status) {
    case 'connected':
      return { color: 'text-tertiary', bg: 'bg-tertiary/10', border: 'border-tertiary/20', dot: 'bg-tertiary', label: 'connected' };
    case 'syncing':
      return { color: 'text-primary-glass', bg: 'bg-primary-glass/10', border: 'border-primary-glass/20', dot: 'bg-primary-glass', label: 'syncing' };
    case 'error':
      return { color: 'text-red-400', bg: 'bg-red-400/10', border: 'border-red-400/20', dot: 'bg-red-400', label: 'error' };
    default:
      return { color: 'text-outline', bg: 'bg-surface-container-high', border: 'border-outline-variant/20', dot: 'bg-outline', label: status || 'not configured' };
  }
}

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

  const allIntegrations = summaries.data ?? [];

  return (
    <motion.div
      variants={containerVariants}
      initial="hidden"
      animate="show"
      className="max-w-6xl mx-auto space-y-6 select-none"
    >
      {/* ── Page Header ─────────────────────────────────── */}
      <motion.div variants={itemVariants} className="flex items-start justify-between gap-4 flex-wrap">
        <div>
          <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-1">
            Source Control
          </p>
          <h2 className="text-3xl font-bold text-on-surface">Integrations and sync health</h2>
          <p className="mt-2 text-[14px] text-on-surface-variant max-w-xl leading-relaxed">
            Connect knowledge systems, trigger indexed syncs, and monitor the readiness of
            future source adapters.
          </p>
        </div>
        <button
          className="flex items-center gap-2 rounded-xl border border-outline-variant/30 bg-surface-container-high/50 px-4 py-2.5 text-[13px] font-medium text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high transition-all"
          onClick={() => summaries.refetch()}
          type="button"
          id="integrations-refresh-btn"
        >
          <RefreshCw size={15} />
          Refresh All
        </button>
      </motion.div>

      <div className="grid gap-5 lg:grid-cols-[1fr_320px]">
        {/* ── Active Integrations ──────────────────────── */}
        <motion.div variants={itemVariants} className="space-y-4">
          {allIntegrations.map((integration) => {
            const isNotion = integration.key === 'notion';
            const isObsidian = integration.key === 'obsidian';
            const isGoogle = integration.key === 'google';
            const sc = statusConfig(integration.status);
            const isCurrentlySyncing = (isNotion && notionSync.isPending) || (isObsidian && obsidianScan.isPending);

            return (
              <motion.div
                key={integration.key}
                whileHover={{ y: -2, borderColor: 'rgba(142, 213, 255, 0.2)' }}
                className="glass-panel rounded-2xl p-6 transition-all"
                id={`integration-${integration.key}`}
              >
                <div className="flex items-start justify-between gap-4 flex-wrap">
                  <div className="flex items-start gap-4 min-w-0">
                    {/* Icon */}
                    <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-[#0b1326]/30 border border-outline-variant/30">
                      <IntegrationIcon kind={integration.key} />
                    </div>

                    {/* Info */}
                    <div className="min-w-0 pt-0.5">
                      <div className="flex items-center gap-3 flex-wrap mb-1">
                        <h3 className="text-lg font-bold text-on-surface">
                          {integration.label}
                        </h3>
                        <span
                          className={`flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 font-mono text-[10px] font-bold ${sc.color} ${sc.bg} ${sc.border}`}
                        >
                          <span className={`h-1.5 w-1.5 rounded-full ${sc.dot} ${isCurrentlySyncing ? 'animate-ai-pulse' : ''}`} />
                          {isCurrentlySyncing ? 'syncing' : sc.label}
                        </span>
                      </div>
                      <p className="text-[13px] text-on-surface-variant">
                        {integration.detail ?? 'Ready to configure and sync.'}
                      </p>
                      {integration.lastSyncedAt && (
                        <p className="mt-1.5 font-mono text-[10px] text-outline">
                          Last sync:{' '}
                          {new Date(integration.lastSyncedAt).toLocaleString('en-US', {
                            month: 'numeric',
                            day: 'numeric',
                            year: 'numeric',
                            hour: '2-digit',
                            minute: '2-digit',
                          })}
                        </p>
                      )}
                    </div>
                  </div>

                  {/* Action buttons */}
                  <div className="flex gap-2 shrink-0">
                    {isNotion && (
                      <button
                        className={`flex items-center gap-2 rounded-xl px-5 py-2.5 text-[13px] font-medium transition-all disabled:opacity-50 active:scale-95 ${
                          notionSync.isPending
                            ? 'border border-primary-glass/20 bg-primary-glass/10 text-primary-glass'
                            : 'border border-outline-variant/30 bg-surface-container-high/50 text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high'
                        }`}
                        disabled={notionSync.isPending}
                        onClick={() => notionSync.mutate()}
                        type="button"
                        id="notion-sync-btn"
                      >
                        {notionSync.isPending ? (
                          <Loader2 size={15} className="animate-spin" />
                        ) : (
                          <RefreshCw size={15} />
                        )}
                        {notionSync.isPending ? 'Syncing…' : 'Sync Now'}
                      </button>
                    )}
                    {isObsidian && (
                      <button
                        className="flex items-center gap-2 rounded-xl border border-outline-variant/30 bg-surface-container-high/50 px-5 py-2.5 text-[13px] font-medium text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high transition-all disabled:opacity-50 active:scale-95"
                        disabled={obsidianScan.isPending}
                        onClick={() => obsidianScan.mutate()}
                        type="button"
                        id="obsidian-scan-btn"
                      >
                        {obsidianScan.isPending ? (
                          <Loader2 size={15} className="animate-spin" />
                        ) : (
                          <FolderOpen size={15} />
                        )}
                        {obsidianScan.isPending ? 'Scanning…' : 'Scan vault'}
                      </button>
                    )}
                    {isGoogle && (
                      <button
                        className="flex items-center gap-2 rounded-xl bg-primary-container px-5 py-2.5 text-[13px] font-medium text-on-primary shadow-lg hover:brightness-110 active:scale-95 transition-all disabled:opacity-50"
                        disabled={googleConnect.isPending}
                        onClick={() => googleConnect.mutate()}
                        type="button"
                        id="google-connect-btn"
                      >
                        {googleConnect.isPending ? (
                          <Loader2 size={15} className="animate-spin" />
                        ) : (
                          <CheckCircle2 size={15} />
                        )}
                        {googleConnect.isPending ? 'Connecting…' : 'Connect'}
                      </button>
                    )}
                  </div>
                </div>

                {/* Progress bar when syncing */}
                <AnimatePresence>
                  {isCurrentlySyncing && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                      className="mt-4 space-y-1.5 overflow-hidden"
                    >
                      <div className="flex justify-between font-mono text-[10px] text-outline">
                        <span>
                          {isNotion ? 'Fetching Notion documents…' : 'Scanning vault files…'}
                        </span>
                        <span>65% complete</span>
                      </div>
                      <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-container-highest/60 relative">
                        <motion.div
                          className="h-full bg-primary-glass rounded-full shadow-[0_0_8px_rgba(142,213,255,0.6)]"
                          initial={{ width: 0 }}
                          animate={{ width: '65%' }}
                          transition={{ duration: 1.5, ease: 'easeOut' }}
                        />
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </motion.div>
            );
          })}
        </motion.div>

        {/* ── Right Column: Roadmap + Stats ────────────── */}
        <motion.div variants={itemVariants} className="space-y-4">
          {/* Roadmap Adapters */}
          <div className="glass-panel rounded-2xl p-5">
            <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-1">
              Roadmap Adapters
            </p>
            <h3 className="text-[16px] font-bold text-on-surface mb-1">Coming soon</h3>
            <p className="text-[12px] text-on-surface-variant mb-5 leading-relaxed">
              Future integrations already accounted for in the information architecture.
            </p>

            <div className="space-y-3">
              <motion.div 
                whileHover={{ x: 2 }} 
                className="flex items-center gap-3 rounded-xl border border-outline-variant/20 bg-surface-container-high/20 p-4 opacity-60 transition-colors hover:bg-surface-container-high/30"
              >
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-surface-container-highest border border-outline-variant/20">
                  <Mail size={18} className="text-outline" />
                </div>
                <div className="min-w-0">
                  <p className="font-medium text-on-surface-variant text-[13px]">Gmail</p>
                  <p className="text-[11px] text-outline leading-relaxed">
                    Thread summaries, inbox retrieval, and grounded email actions.
                  </p>
                </div>
              </motion.div>

              <motion.div 
                whileHover={{ x: 2 }} 
                className="flex items-center gap-3 rounded-xl border border-outline-variant/20 bg-surface-container-high/20 p-4 opacity-60 transition-colors hover:bg-surface-container-high/30"
              >
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-surface-container-highest border border-outline-variant/20">
                  <Calendar size={18} className="text-outline" />
                </div>
                <div className="min-w-0">
                  <p className="font-medium text-on-surface-variant text-[13px]">Calendar</p>
                  <p className="text-[11px] text-outline leading-relaxed">
                    Time-aware retrieval and schedule-assisted planning.
                  </p>
                </div>
              </motion.div>

              <div className="flex items-center gap-3 rounded-xl border border-dashed border-outline-variant/20 bg-[#0b1326]/10 p-4">
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-dashed border-outline-variant/30">
                  <Plus size={18} className="text-outline" />
                </div>
                <div className="min-w-0">
                  <p className="font-medium text-on-surface-variant text-[13px]">
                    Request a custom connector
                  </p>
                </div>
              </div>
            </div>
          </div>

          {/* Sync Stats */}
          <div className="glass-panel rounded-2xl p-5">
            <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-4">
              Sync Stats
            </p>
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-[13px] text-on-surface-variant">Connected</span>
                <span className="font-mono text-[14px] text-tertiary font-bold">
                  {allIntegrations.filter((i) => i.status === 'connected').length} /{' '}
                  {allIntegrations.length}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-[13px] text-on-surface-variant">Total adapters</span>
                <span className="font-mono text-[14px] text-on-surface font-bold">
                  {allIntegrations.length + 2}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-[13px] text-on-surface-variant">Roadmap</span>
                <span className="font-mono text-[14px] text-outline font-bold">2 planned</span>
              </div>
            </div>
          </div>
        </motion.div>
      </div>
    </motion.div>
  );
}
