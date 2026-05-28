import { useState } from 'react';
import { Database, Save, RotateCcw, CheckCircle2, Shield, RefreshCw } from 'lucide-react';

import { ErrorState } from '@/components/states/error-state';
import { LoadingState } from '@/components/states/loading-state';
import { useSettingsQuery } from '@/features/settings/hooks/use-settings-query';
import { useUpdateSettingsMutation } from '@/features/settings/hooks/use-update-settings-mutation';

const DEFAULT_VAULT_PATH = '/Users/saumyathacker/Documents/rag_sys/rag_sys';

export function SettingsPage() {
  const settingsQuery = useSettingsQuery();
  const updateSettings = useUpdateSettingsMutation();
  const [draftPath, setDraftPath] = useState('');

  const currentPath = (settingsQuery.data as any)?.obsidian_vault_path ?? DEFAULT_VAULT_PATH;
  const draftValue = draftPath || currentPath;
  const isDirty = draftPath.length > 0 && draftPath !== currentPath;

  function handleSave() {
    updateSettings.mutate(
      { obsidian_vault_path: draftValue } as any,
      { onSuccess: () => setDraftPath('') },
    );
  }

  if (settingsQuery.isLoading) {
    return <LoadingState label="Loading configuration from the local settings store." />;
  }

  if (settingsQuery.isError) {
    return (
      <ErrorState
        description="The desktop app could not read vault configuration from its SQLite store."
        onRetry={() => settingsQuery.refetch()}
        title="Failed to load settings"
      />
    );
  }

  return (
    <div className="max-w-4xl mx-auto animate-slide-up">
      {/* ── Page Header ─────────────────────────────────── */}
      <div className="mb-8">
        <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-1">
          Configuration
        </p>
        <h2 className="text-3xl font-bold text-on-surface">Settings</h2>
      </div>

      <div className="space-y-5">
        {/* ── Obsidian Vault Path ──────────────────────── */}
        <section className="glass-panel rounded-2xl p-8">
          <div className="flex items-start gap-5 mb-7">
            <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-surface-container-high border border-outline-variant/30">
              <Database size={24} className="text-primary-glass" />
            </div>
            <div>
              <h3 className="text-xl font-bold text-on-surface mb-1">Obsidian vault path</h3>
              <p className="text-[13px] text-on-surface-variant leading-relaxed max-w-xl">
                Point the desktop app at the local knowledge vault you want indexed for RAG
                retrieval and citation processing.
              </p>
            </div>
          </div>

          {/* Current path display */}
          <div className="mb-5">
            <label className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-2 block">
              Current Path
            </label>
            <div className="flex items-center gap-3 rounded-xl border border-outline-variant/20 bg-surface-container-high/30 px-4 py-3">
              <Database size={16} className="text-outline shrink-0" />
              <span className="font-mono text-[13px] text-on-surface-variant truncate">
                {currentPath}
              </span>
            </div>
          </div>

          {/* Editable path */}
          <div className="mb-6">
            <label
              className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-2 block"
              htmlFor="vault-path-input"
            >
              New Vault Path
            </label>
            <div
              className={`flex items-center gap-3 rounded-xl border px-4 py-3 transition-colors ${
                isDirty
                  ? 'border-primary-glass/40 bg-primary-glass/5'
                  : 'border-outline-variant/30 bg-surface-container-high/30'
              }`}
            >
              <Database size={16} className="text-outline shrink-0" />
              <input
                className="flex-1 bg-transparent font-mono text-[13px] text-on-surface focus:outline-none placeholder:text-outline min-w-0"
                id="vault-path-input"
                onChange={(e) => setDraftPath(e.target.value)}
                placeholder={currentPath}
                value={draftValue}
              />
            </div>
            <p className="mt-2 text-[12px] text-outline">
              Standard Unix path format. Symlinks are supported but not recommended for indexing
              stability.
            </p>
          </div>

          {/* Actions */}
          <div className="flex flex-wrap items-center justify-between gap-4">
            <div className="flex gap-3">
              <button
                className="flex items-center gap-2 rounded-xl bg-primary-container px-5 py-2.5 text-[13px] font-medium text-on-primary shadow-lg hover:brightness-110 active:scale-95 transition-all disabled:opacity-50"
                disabled={updateSettings.isPending || !isDirty}
                onClick={handleSave}
                type="button"
                id="settings-save-vault-btn"
              >
                <Save size={15} />
                Save vault path
              </button>
              <button
                className="flex items-center gap-2 rounded-xl border border-outline-variant/30 bg-surface-container-high/40 px-5 py-2.5 text-[13px] text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high transition-all"
                onClick={() => setDraftPath('')}
                type="button"
                id="settings-reset-vault-btn"
              >
                <RotateCcw size={15} />
                Reset to default
              </button>
            </div>

            {!isDirty && !updateSettings.isPending && (
              <div className="flex items-center gap-2 text-tertiary">
                <CheckCircle2 size={16} />
                <span className="text-[13px] font-medium">Vault validated &amp; ready</span>
              </div>
            )}
          </div>
        </section>

        {/* ── Bottom Row: Auto-sync + Key Management ─── */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
          {/* Auto-sync Frequency */}
          <section className="glass-panel rounded-2xl p-6">
            <div className="flex items-center gap-3 mb-5">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-surface-container-high border border-outline-variant/30">
                <RefreshCw size={18} className="text-primary-glass" />
              </div>
              <h3 className="text-[16px] font-bold text-on-surface">Auto-sync Frequency</h3>
            </div>
            <p className="text-[13px] text-on-surface-variant mb-5 leading-relaxed">
              Configure how often sources are automatically polled for new content.
            </p>
            <div className="space-y-2">
              {['Every 15 minutes', 'Every hour', 'Every 6 hours', 'Manual only'].map(
                (option) => (
                  <label
                    key={option}
                    className="flex items-center gap-3 rounded-xl px-4 py-3 border border-transparent hover:border-outline-variant/20 hover:bg-surface-container-high/30 transition-all cursor-pointer group"
                  >
                    <input
                      type="radio"
                      name="sync-frequency"
                      value={option}
                      className="accent-[color:var(--primary-glass)] cursor-pointer"
                      defaultChecked={option === 'Every hour'}
                    />
                    <span className="text-[13px] text-on-surface-variant group-hover:text-on-surface transition-colors">
                      {option}
                    </span>
                  </label>
                ),
              )}
            </div>
          </section>

          {/* Key Management */}
          <section className="glass-panel rounded-2xl p-6">
            <div className="flex items-center gap-3 mb-5">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-surface-container-high border border-outline-variant/30">
                <Shield size={18} className="text-primary-glass" />
              </div>
              <h3 className="text-[16px] font-bold text-on-surface">Key Management</h3>
            </div>
            <p className="text-[13px] text-on-surface-variant mb-5 leading-relaxed">
              Manage API keys and authentication tokens stored in the local secure vault.
            </p>

            <div className="space-y-3">
              {[
                { label: 'Google OAuth', status: 'Configured' },
                { label: 'Notion API Key', status: 'Configured' },
                { label: 'Ollama Endpoint', status: 'Local' },
              ].map((item) => (
                <div
                  key={item.label}
                  className="flex items-center justify-between rounded-xl border border-outline-variant/20 bg-surface-container-high/30 px-4 py-3"
                >
                  <span className="text-[13px] text-on-surface-variant">{item.label}</span>
                  <span className="font-mono text-[10px] rounded-full bg-tertiary/10 border border-tertiary/20 text-tertiary px-2 py-0.5 font-bold">
                    {item.status}
                  </span>
                </div>
              ))}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
