import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { ArrowRight, Check, Loader2, ShieldAlert, Key, FolderOpen, RefreshCw } from 'lucide-react';
import { useOnboarding, type SourceKey } from '../stores/onboarding-context';
import { invokeCommand } from '@/lib/api/invoke-command';
import { useIntegrationSummariesQuery } from '@/features/integrations/hooks/use-integration-summaries-query';
import { useSettingsQuery } from '@/features/settings/hooks/use-settings-query';
import { useGoogleConnectMutation } from '@/features/integrations/hooks/use-google-connect-mutation';

import {
  NotionLogo,
  DriveLogo,
  GmailLogo,
  CalendarLogo,
  AppleLogo,
  ObsidianLogo,
  FolderLogo,
} from './logos-helper';

const sourceItems: { key: SourceKey; label: string; sub: string; Icon: any }[] = [
  { key: 'notion', label: 'Notion', sub: 'Workspaces, pages, databases', Icon: NotionLogo },
  { key: 'obsidian', label: 'Obsidian', sub: 'Point at any markdown vault', Icon: ObsidianLogo },
  { key: 'local', label: 'Local files', sub: 'Any folder on disk', Icon: FolderLogo },
  { key: 'drive', label: 'Google Drive', sub: 'Docs, sheets, slides, PDFs', Icon: DriveLogo },
  { key: 'gmail', label: 'Gmail', sub: 'Inbox, threads, attachments', Icon: GmailLogo },
  { key: 'gcal', label: 'Google Calendar', sub: 'Events, agendas, notes', Icon: CalendarLogo },
  { key: 'apple-cal', label: 'Apple Calendar', sub: 'Local EventKit access', Icon: AppleLogo },
];

export function SourcesStep() {
  const { sources: selected, addSource, removeSource, toggleSource } = useOnboarding();
  const integrations = useIntegrationSummariesQuery();
  const settings = useSettingsQuery();
  const googleConnect = useGoogleConnectMutation();

  const [activeConfiguring, setActiveConfiguring] = useState<SourceKey | null>(null);
  
  // Inputs
  const [notionToken, setNotionToken] = useState('');
  const [obsidianPath, setObsidianPath] = useState('');
  const [localPath, setLocalPath] = useState('');

  // Statuses
  const [saving, setSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState('');
  const [successMsg, setSuccessMsg] = useState('');

  const [hasInitialized, setHasInitialized] = useState(false);
  const [localSaved, setLocalSaved] = useState(false);

  // Sync existing configs from DB on mount once
  useEffect(() => {
    if (hasInitialized) return;
    if (integrations.isSuccess && settings.isSuccess) {
      const notionInt = integrations.data.find(i => i.key === 'notion');
      if (notionInt && notionInt.status === 'connected') {
        addSource('notion');
      }

      const googleInt = integrations.data.find(i => i.key === 'google');
      if (googleInt && googleInt.status === 'connected') {
        addSource('drive');
        addSource('gmail');
        addSource('gcal');
      }

      const path = (settings.data as any)?.obsidian_vault_path || (settings.data as any)?.obsidianVaultPath;
      if (path) {
        setObsidianPath(path);
        addSource('obsidian');
      }
      setHasInitialized(true);
    }
  }, [integrations.isSuccess, settings.isSuccess, hasInitialized]);

  const isDbConnected = (key: SourceKey): boolean => {
    if (key === 'notion') {
      return integrations.data?.find(i => i.key === 'notion')?.status === 'connected';
    }
    if (key === 'obsidian') {
      const path = (settings.data as any)?.obsidian_vault_path || (settings.data as any)?.obsidianVaultPath;
      return !!path;
    }
    if (key === 'local') {
      return localSaved;
    }
    if (key === 'drive' || key === 'gmail' || key === 'gcal') {
      return integrations.data?.find(i => i.key === 'google')?.status === 'connected';
    }
    if (key === 'apple-cal') {
      return selected.includes('apple-cal');
    }
    return false;
  };

  const handleSaveNotion = async () => {
    if (!notionToken.trim()) return;
    setSaving(true);
    setErrorMsg('');
    setSuccessMsg('');
    try {
      await invokeCommand('save_credential', { provider: 'notion', token: notionToken.trim() });
      addSource('notion');
      setSuccessMsg('Notion integration token saved securely.');
      setNotionToken('');
      integrations.refetch();
    } catch (err: any) {
      setErrorMsg(err?.message || 'Failed to save Notion token.');
    } finally {
      setSaving(false);
    }
  };

  const handleSaveObsidian = async () => {
    if (!obsidianPath.trim()) return;
    setSaving(true);
    setErrorMsg('');
    setSuccessMsg('');
    try {
      await invokeCommand('select_obsidian_vault', { path: obsidianPath.trim() });
      addSource('obsidian');
      setSuccessMsg('Obsidian vault path updated.');
      settings.refetch();
      integrations.refetch();
    } catch (err: any) {
      setErrorMsg(err?.message || 'Failed to save Obsidian vault path.');
    } finally {
      setSaving(false);
    }
  };

  const handleSaveLocal = async () => {
    if (!localPath.trim()) return;
    setSaving(true);
    setErrorMsg('');
    setSuccessMsg('');
    try {
      await invokeCommand('save_credential', { provider: 'local', token: localPath.trim() });
      addSource('local');
      setLocalSaved(true);
      setSuccessMsg('Local folder path saved.');
      integrations.refetch();
    } catch (err: any) {
      setErrorMsg(err?.message || 'Failed to save local path.');
    } finally {
      setSaving(false);
    }
  };

  const handleGoogleAuth = async () => {
    setErrorMsg('');
    setSuccessMsg('');
    try {
      googleConnect.mutate(undefined, {
        onSuccess: () => {
          setSuccessMsg('Google authentication initiated in browser.');
          addSource('drive');
          addSource('gmail');
          addSource('gcal');
          integrations.refetch();
        },
        onError: (err: any) => {
          setErrorMsg(err?.message || 'Failed to connect Google account.');
        }
      });
    } catch (err: any) {
      setErrorMsg(err?.message || 'Failed to initiate Google OAuth.');
    }
  };

  const isConfigured = (key: SourceKey): boolean => {
    return selected.includes(key);
  };

  const handleCardClick = (key: SourceKey) => {
    toggleSource(key);
    setActiveConfiguring(activeConfiguring === key ? null : key);
  };

  return (
    <div>
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4 }}
      >
        <h1 className="text-3xl font-bold text-gradient md:text-4xl">
          Welcome. Connect your knowledge sources.
        </h1>
        <p className="mt-3 text-sm text-outline md:text-base">
          Select and configure the sources you'd like Lumen to index. Credentials are encrypted and stored in your local SQLite database.
        </p>
      </motion.div>

      {/* Grid of Sources */}
      <div className="mt-8 grid gap-3 sm:grid-cols-2">
        {sourceItems.map((s, i) => {
          const configured = isConfigured(s.key);
          const configuring = activeConfiguring === s.key;
          const dbConnected = isDbConnected(s.key);
          return (
            <motion.button
              key={s.key}
              type="button"
              onClick={() => handleCardClick(s.key)}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3, delay: i * 0.04 }}
              className={`group relative flex items-center gap-4 rounded-2xl border p-4 text-left transition-all ${
                configuring
                  ? 'border-primary-glass bg-primary-glass/5 glow-sm'
                  : configured
                  ? 'border-primary-glass/40 bg-surface-container-high/40 hover:bg-surface-container-high/60'
                  : 'border-outline-variant/20 bg-surface-container-lowest/20 hover:bg-surface-container-high/30'
              }`}
            >
              <div
                className={`flex h-11 w-11 items-center justify-center rounded-xl transition-colors ${
                  configured ? 'bg-primary-glass/15 text-primary-glass' : 'bg-surface-container-high text-outline'
                }`}
              >
                <s.Icon className="h-5 w-5" />
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-sm font-bold text-white flex items-center gap-2">
                  {s.label}
                  {dbConnected && (
                    <span className="h-1.5 w-1.5 rounded-full bg-green-400 animate-ai-pulse" />
                  )}
                </div>
                <div className="text-xs text-outline truncate">{s.sub}</div>
              </div>
              <div
                className={`flex h-5 w-5 items-center justify-center rounded-full border transition-all ${
                  configured
                    ? 'border-primary-glass bg-primary-glass text-black'
                    : 'border-outline-variant bg-transparent'
                }`}
              >
                {configured && <Check className="h-3.5 w-3.5 stroke-[3]" />}
              </div>
            </motion.button>
          );
        })}
      </div>

      {/* Expanded Configuration Section */}
      <AnimatePresence mode="wait">
        {activeConfiguring && (
          <motion.div
            key={activeConfiguring}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            className="mt-6 overflow-hidden"
          >
            <div className="glass-strong rounded-2xl p-6 relative border border-primary-glass/20">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-[15px] font-bold text-white uppercase tracking-wider flex items-center gap-2">
                  <Key size={16} className="text-primary-glass" />
                  Configure {sourceItems.find(s => s.key === activeConfiguring)?.label}
                </h3>
                {isDbConnected(activeConfiguring) && (
                  <span className="text-[11px] font-mono uppercase bg-green-500/10 text-green-400 border border-green-500/20 px-2 py-0.5 rounded-full">
                    Connected
                  </span>
                )}
              </div>

              {activeConfiguring === 'notion' && (
                <div className="space-y-4">
                  <p className="text-xs text-outline leading-relaxed">
                    Paste your Notion Integration Token (secret_*). Your token is saved encrypted locally.
                  </p>
                  <div className="flex gap-2">
                    <input
                      type="password"
                      placeholder="secret_..."
                      value={notionToken}
                      onChange={e => setNotionToken(e.target.value)}
                      className="flex-1 rounded-xl border border-outline-variant/30 bg-surface-container-high/40 px-4 py-2.5 text-xs text-white focus:outline-none focus:border-primary-glass font-mono"
                    />
                    <button
                      type="button"
                      disabled={saving || !notionToken}
                      onClick={handleSaveNotion}
                      className="rounded-xl bg-primary-glass px-4 py-2.5 text-xs font-bold text-black hover:brightness-110 disabled:opacity-50 flex items-center gap-1.5 transition-all"
                    >
                      {saving && <Loader2 size={13} className="animate-spin" />}
                      Save
                    </button>
                  </div>
                </div>
              )}

              {activeConfiguring === 'obsidian' && (
                <div className="space-y-4">
                  <p className="text-xs text-outline leading-relaxed">
                    Specify the absolute path to your Obsidian vault.symlinks/folders.
                  </p>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      placeholder="/Users/username/Documents/ObsidianVault"
                      value={obsidianPath}
                      onChange={e => setObsidianPath(e.target.value)}
                      className="flex-1 rounded-xl border border-outline-variant/30 bg-surface-container-high/40 px-4 py-2.5 text-xs text-white focus:outline-none focus:border-primary-glass font-mono"
                    />
                    <button
                      type="button"
                      disabled={saving || !obsidianPath}
                      onClick={handleSaveObsidian}
                      className="rounded-xl bg-primary-glass px-4 py-2.5 text-xs font-bold text-black hover:brightness-110 disabled:opacity-50 flex items-center gap-1.5 transition-all"
                    >
                      {saving && <Loader2 size={13} className="animate-spin" />}
                      Save
                    </button>
                  </div>
                </div>
              )}

              {activeConfiguring === 'local' && (
                <div className="space-y-4">
                  <p className="text-xs text-outline leading-relaxed">
                    Enter the absolute folder path containing local files (PDFs, Markdown, text docs) to monitor and index.
                  </p>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      placeholder="/Users/username/Downloads/MyFolder"
                      value={localPath}
                      onChange={e => setLocalPath(e.target.value)}
                      className="flex-1 rounded-xl border border-outline-variant/30 bg-surface-container-high/40 px-4 py-2.5 text-xs text-white focus:outline-none focus:border-primary-glass font-mono"
                    />
                    <button
                      type="button"
                      disabled={saving || !localPath}
                      onClick={handleSaveLocal}
                      className="rounded-xl bg-primary-glass px-4 py-2.5 text-xs font-bold text-black hover:brightness-110 disabled:opacity-50 flex items-center gap-1.5 transition-all"
                    >
                      {saving && <Loader2 size={13} className="animate-spin" />}
                      Save
                    </button>
                  </div>
                </div>
              )}

              {['drive', 'gmail', 'gcal'].includes(activeConfiguring) && (
                <div className="space-y-4">
                  <p className="text-xs text-outline leading-relaxed">
                    Connecting one Google service authenticates all Drive, Gmail, and Calendar sources. Complete the secure Google browser login popup.
                  </p>
                  <button
                    type="button"
                    disabled={googleConnect.isPending}
                    onClick={handleGoogleAuth}
                    className="rounded-xl bg-primary-glass px-5 py-3 text-xs font-bold text-black hover:brightness-110 disabled:opacity-50 flex items-center gap-2 transition-all shadow-lg"
                  >
                    {googleConnect.isPending ? (
                      <Loader2 size={14} className="animate-spin" />
                    ) : (
                      <FolderOpen size={14} />
                    )}
                    Connect Google Account
                  </button>
                </div>
              )}

              {activeConfiguring === 'apple-cal' && (
                <div className="space-y-2">
                  <p className="text-xs text-outline">
                    Uses local EventKit permission settings. This requires native OS approval when requested by the application.
                  </p>
                  <button
                    type="button"
                    onClick={() => {
                      addSource('apple-cal');
                      setSuccessMsg('Apple Calendar selected.');
                    }}
                    className="rounded-xl border border-outline-variant/30 bg-surface-container-high/50 hover:bg-surface-container-high px-4 py-2 text-xs font-bold text-white transition-all"
                  >
                    Select Calendar Source
                  </button>
                </div>
              )}

              {/* Status notifications */}
              {errorMsg && (
                <div className="mt-4 flex items-center gap-2 text-red-400 text-xs font-mono">
                  <ShieldAlert size={14} />
                  {errorMsg}
                </div>
              )}
              {successMsg && (
                <div className="mt-4 flex items-center gap-2 text-green-400 text-xs font-mono">
                  <Check size={14} />
                  {successMsg}
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Navigation */}
      <div className="mt-10 flex items-center justify-between">
        <div className="text-xs text-outline">
          {selected.filter(key => isDbConnected(key)).length} source{selected.filter(key => isDbConnected(key)).length === 1 ? '' : 's'} connected
        </div>
        <Link
          to="/onboarding/ai-setup"
          className={`group inline-flex items-center gap-2 rounded-full bg-primary-glass px-6 py-3 text-sm font-bold text-black transition-all hover:glow-sm ${
            selected.length === 0 || !selected.every(key => isDbConnected(key)) ? 'opacity-50 pointer-events-none' : ''
          }`}
        >
          Continue
          <ArrowRight className="h-4 w-4 transition-transform group-hover:translate-x-0.5" />
        </Link>
      </div>
    </div>
  );
}
