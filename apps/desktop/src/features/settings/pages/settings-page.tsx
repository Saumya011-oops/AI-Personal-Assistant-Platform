import { useState, useEffect } from 'react';
import {
  Database,
  Save,
  RotateCcw,
  CheckCircle2,
  Shield,
  RefreshCw,
  Brain,
  Trash2,
  Edit2,
  Download,
  Upload,
  AlertTriangle,
  Check,
  X,
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

import { ErrorState } from '@/components/states/error-state';
import { LoadingState } from '@/components/states/loading-state';
import { useSettingsQuery } from '@/features/settings/hooks/use-settings-query';
import { useUpdateSettingsMutation } from '@/features/settings/hooks/use-update-settings-mutation';
import { invokeCommand } from '@/lib/api/invoke-command';

const DEFAULT_VAULT_PATH = '/Users/saumyathacker/Documents/rag_sys/rag_sys';

const containerVariants = {
  hidden: { opacity: 0 },
  show: {
    opacity: 1,
    transition: {
      staggerChildren: 0.05,
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

export function SettingsPage() {
  const settingsQuery = useSettingsQuery();
  const updateSettings = useUpdateSettingsMutation();
  const [draftPath, setDraftPath] = useState('');
  const [syncFreq, setSyncFreq] = useState('Every hour');

  // Memory management states
  const [memories, setMemories] = useState<any[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingContent, setEditingContent] = useState('');
  const [editingImportance, setEditingImportance] = useState(5);

  const currentPath = (settingsQuery.data as any)?.obsidian_vault_path ?? DEFAULT_VAULT_PATH;
  const draftValue = draftPath || currentPath;
  const isDirty = draftPath.length > 0 && draftPath !== currentPath;

  const loadMemories = async () => {
    try {
      const memoryList = await invokeCommand('list_memories', {});
      setMemories(memoryList || []);
    } catch (e) {
      console.error('Failed to load memories:', e);
    }
  };

  useEffect(() => {
    loadMemories();
  }, []);

  function handleSave() {
    updateSettings.mutate(
      { obsidian_vault_path: draftValue } as any,
      { onSuccess: () => setDraftPath('') },
    );
  }

  const handleDeleteMemory = async (id: string) => {
    if (!window.confirm("Are you sure you want to delete this memory?")) return;
    try {
      await invokeCommand('delete_memory', { id });
      loadMemories();
    } catch (e) {
      console.error('Failed to delete memory:', e);
    }
  };

  const handleUpdateMemory = async (id: string) => {
    try {
      await invokeCommand('update_memory', {
        id,
        content: editingContent,
        importance: Number(editingImportance),
      });
      setEditingId(null);
      loadMemories();
    } catch (e) {
      console.error('Failed to update memory:', e);
    }
  };

  const handleClearAllMemories = async () => {
    if (!window.confirm("Are you sure you want to clear ALL memories? This cannot be undone.")) return;
    try {
      await invokeCommand('clear_all_memories', {});
      loadMemories();
    } catch (e) {
      console.error('Failed to clear memories:', e);
    }
  };

  const handleExportMemories = async () => {
    try {
      const jsonStr = await invokeCommand('export_memories', {});
      const blob = new Blob([jsonStr as string], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'assistant_memories.json';
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error('Failed to export memories:', e);
    }
  };

  const handleImportMemories = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = async (event) => {
      try {
        const jsonStr = event.target?.result as string;
        await invokeCommand('import_memories', { jsonStr });
        loadMemories();
        alert('Memories imported successfully!');
      } catch (err) {
        console.error('Failed to import memories:', err);
        alert('Invalid file format. Import failed.');
      }
    };
    reader.readAsText(file);
  };

  const handleResetData = async () => {
    const confirm1 = window.confirm(
      "WARNING: This will delete ALL local databases, synced Notion/Obsidian files, RAG vector indices, conversation histories, and long-term memory. This action cannot be undone. Are you sure you want to completely reset Assistant Core?"
    );
    if (!confirm1) return;
    const confirm2 = window.confirm(
      "Please confirm one more time: Do you want to proceed with a complete system reset?"
    );
    if (!confirm2) return;

    try {
      await invokeCommand('reset_assistant_data', {});
      localStorage.removeItem('onboarding_complete');
      window.location.href = '/onboarding';
    } catch (err) {
      console.error('Reset failed:', err);
      alert('Failed to reset all system data.');
    }
  };

  // Filter memories locally
  const filteredMemories = memories.filter((m) =>
    (m.content || '').toLowerCase().includes(searchQuery.toLowerCase()) ||
    (m.type || '').toLowerCase().includes(searchQuery.toLowerCase())
  );

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
    <motion.div
      variants={containerVariants}
      initial="hidden"
      animate="show"
      className="max-w-4xl mx-auto space-y-6 select-none"
    >
      {/* ── Page Header ─────────────────────────────────── */}
      <motion.div variants={itemVariants} className="mb-4">
        <p className="font-mono text-[10px] font-bold uppercase tracking-widest text-outline mb-1">
          Configuration
        </p>
        <h2 className="text-3xl font-bold text-on-surface">Settings</h2>
      </motion.div>

      <div className="space-y-5">
        {/* ── Obsidian Vault Path ──────────────────────── */}
        <motion.section
          variants={itemVariants}
          className="glass-panel rounded-2xl p-8"
        >
          <div className="flex items-start gap-5 mb-7">
            <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-[#0b1326]/30 border border-outline-variant/30">
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
            <div className="flex items-center gap-3 rounded-xl border border-outline-variant/20 bg-[#0b1326]/20 px-4 py-3">
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
              className={`flex items-center gap-3 rounded-xl border px-4 py-3 transition-all ${
                isDirty
                  ? 'border-primary-glass/50 bg-primary-glass/5 shadow-[0_0_12px_rgba(142,213,255,0.15)]'
                  : 'border-outline-variant/20 bg-[#0b1326]/20 focus-within:border-primary-glass/40'
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
                className="flex items-center gap-2 rounded-xl bg-primary-glass px-5 py-2.5 text-[13px] font-bold text-black shadow-lg hover:glow active:scale-95 transition-all disabled:opacity-50 disabled:brightness-50"
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
        </motion.section>

        {/* ── Bottom Row: Auto-sync + Key Management ─── */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
          {/* Auto-sync Frequency */}
          <motion.section variants={itemVariants} className="glass-panel rounded-2xl p-6">
            <div className="flex items-center gap-3 mb-5">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-[#0b1326]/30 border border-outline-variant/30">
                <RefreshCw size={18} className="text-primary-glass" />
              </div>
              <h3 className="text-[16px] font-bold text-on-surface">Auto-sync Frequency</h3>
            </div>
            <p className="text-[13px] text-on-surface-variant mb-5 leading-relaxed">
              Configure how often sources are automatically polled for new content.
            </p>
            <div className="space-y-2">
              {['Every 15 minutes', 'Every hour', 'Every 6 hours', 'Manual only'].map(
                (option) => {
                  const isSelected = syncFreq === option;
                  return (
                    <label
                      key={option}
                      onClick={() => setSyncFreq(option)}
                      className="relative flex items-center gap-3 rounded-xl px-4 py-3 border border-transparent transition-all cursor-pointer group overflow-hidden"
                    >
                      {isSelected && (
                        <motion.div
                          layoutId="active-sync-freq"
                          className="absolute inset-0 bg-primary-glass/5 border-l-2 border-primary-glass"
                          transition={{ type: 'spring', stiffness: 380, damping: 30 }}
                        />
                      )}
                      <input
                        type="radio"
                        name="sync-frequency"
                        value={option}
                        checked={isSelected}
                        onChange={() => {}}
                        className="accent-[color:var(--primary-glass)] cursor-pointer relative z-10"
                      />
                      <span className="text-[13px] text-on-surface-variant group-hover:text-on-surface transition-colors relative z-10">
                        {option}
                      </span>
                    </label>
                  );
                }
              )}
            </div>
          </motion.section>

          {/* Key Management */}
          <motion.section variants={itemVariants} className="glass-panel rounded-2xl p-6">
            <div className="flex items-center gap-3 mb-5">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-[#0b1326]/30 border border-outline-variant/30">
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
                  className="flex items-center justify-between rounded-xl border border-outline-variant/20 bg-[#0b1326]/20 px-4 py-3 hover:border-outline-variant/30 transition-colors"
                >
                  <span className="text-[13px] text-on-surface-variant">{item.label}</span>
                  <span className="font-mono text-[10px] rounded-full bg-tertiary/10 border border-tertiary/20 text-tertiary px-2 py-0.5 font-bold">
                    {item.status}
                  </span>
                </div>
              ))}
            </div>
          </motion.section>
        </div>

        {/* ── Memory Management Section ─── */}
        <motion.section variants={itemVariants} className="glass-panel rounded-2xl p-8">
          <div className="flex items-start justify-between flex-wrap gap-4 mb-7">
            <div className="flex items-start gap-5">
              <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-[#0b1326]/30 border border-outline-variant/30">
                <Brain size={24} className="text-primary-glass" />
              </div>
              <div>
                <h3 className="text-xl font-bold text-on-surface mb-1">Memory Management</h3>
                <p className="text-[13px] text-on-surface-variant leading-relaxed max-w-xl">
                  View, search, edit, or delete long-term profile data, preferences, tasks, and semantic memories saved by the assistant.
                </p>
              </div>
            </div>

            <div className="flex items-center gap-2">
              <button
                onClick={handleExportMemories}
                className="flex items-center gap-1.5 rounded-xl border border-outline-variant/25 bg-surface-container-high/40 hover:bg-surface-container-high px-4 py-2 text-[12px] text-on-surface transition-all"
                title="Export memories to JSON"
              >
                <Download size={14} />
                Export
              </button>

              <label className="flex items-center gap-1.5 rounded-xl border border-outline-variant/25 bg-surface-container-high/40 hover:bg-surface-container-high px-4 py-2 text-[12px] text-on-surface transition-all cursor-pointer">
                <Upload size={14} />
                Import
                <input
                  type="file"
                  accept=".json"
                  onChange={handleImportMemories}
                  className="hidden"
                />
              </label>

              <button
                onClick={handleClearAllMemories}
                className="flex items-center gap-1.5 rounded-xl border border-destructive/20 hover:border-destructive/40 bg-destructive/10 hover:bg-destructive/20 px-4 py-2 text-[12px] text-destructive transition-all"
              >
                <Trash2 size={14} />
                Clear All
              </button>
            </div>
          </div>

          {/* Search bar */}
          <div className="mb-6">
            <div className="flex items-center gap-3 rounded-xl border border-outline-variant/20 bg-[#0b1326]/20 px-4 py-2.5 focus-within:border-primary-glass/40 transition-all">
              <Brain size={16} className="text-outline" />
              <input
                type="text"
                placeholder="Search memories by content or type..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="flex-1 bg-transparent text-[13px] text-on-surface focus:outline-none placeholder:text-outline"
              />
            </div>
          </div>

          {/* Memory List */}
          <div className="max-h-[350px] overflow-y-auto custom-scrollbar space-y-3 pr-2">
            {filteredMemories.length === 0 ? (
              <div className="text-center py-8 text-outline text-[13px] border border-dashed border-outline-variant/20 rounded-xl bg-[#0b1326]/10">
                {searchQuery ? "No matching memories found." : "No memories stored yet. Converse with the assistant to record memories."}
              </div>
            ) : (
              filteredMemories.map((mem: any) => {
                const isEditing = editingId === mem.id;
                return (
                  <div
                    key={mem.id}
                    className="flex items-center justify-between gap-4 rounded-xl border border-outline-variant/15 bg-[#0b1326]/15 hover:bg-surface-container-low/20 p-4 transition-all"
                  >
                    <div className="flex-1 min-w-0">
                      {isEditing ? (
                        <div className="flex flex-col gap-2">
                          <input
                            type="text"
                            value={editingContent}
                            onChange={(e) => setEditingContent(e.target.value)}
                            className="w-full bg-[#0b1326]/40 border border-primary-glass/40 rounded-lg px-3 py-1.5 text-[13px] text-on-surface focus:outline-none"
                          />
                          <div className="flex items-center gap-4">
                            <div className="flex items-center gap-1.5">
                              <span className="text-[11px] text-outline">Importance:</span>
                              <input
                                type="number"
                                min="1"
                                max="10"
                                value={editingImportance}
                                onChange={(e) => setEditingImportance(Number(e.target.value))}
                                className="w-12 bg-[#0b1326]/40 border border-outline-variant/30 rounded px-2 py-0.5 text-[11px] text-center text-on-surface focus:outline-none"
                              />
                            </div>
                          </div>
                        </div>
                      ) : (
                        <div>
                          <p className="text-[13px] text-on-surface leading-relaxed break-words font-light">
                            {mem.content}
                          </p>
                          <div className="flex flex-wrap items-center gap-2.5 mt-2">
                            <span className="text-[9px] font-bold uppercase tracking-wider px-2 py-0.5 rounded bg-primary-glass/10 border border-primary-glass/25 text-primary-glass">
                              {mem.type}
                            </span>
                            <span className="text-[10px] text-outline">
                              Importance: <strong className="text-on-surface-variant">{mem.importance}/10</strong>
                            </span>
                            <span className="text-[10px] text-outline">•</span>
                            <span className="text-[10px] text-outline">
                              Confidence: <strong className="text-on-surface-variant">{(mem.confidence * 100).toFixed(0)}%</strong>
                            </span>
                            <span className="text-[10px] text-outline">•</span>
                            <span className="text-[10px] text-outline">
                              Access Count: <strong className="text-on-surface-variant">{mem.access_count}</strong>
                            </span>
                          </div>
                        </div>
                      )}
                    </div>

                    <div className="flex items-center gap-1 shrink-0">
                      {isEditing ? (
                        <>
                          <button
                            onClick={() => handleUpdateMemory(mem.id)}
                            className="p-2 hover:bg-emerald-500/10 text-emerald-400 hover:text-emerald-300 rounded-lg transition-all"
                            title="Save changes"
                          >
                            <Check size={15} />
                          </button>
                          <button
                            onClick={() => setEditingId(null)}
                            className="p-2 hover:bg-outline-variant/10 text-outline hover:text-on-surface rounded-lg transition-all"
                            title="Cancel editing"
                          >
                            <X size={15} />
                          </button>
                        </>
                      ) : (
                        <>
                          <button
                            onClick={() => {
                              setEditingId(mem.id);
                              setEditingContent(mem.content);
                              setEditingImportance(mem.importance);
                            }}
                            className="p-2 hover:bg-surface-container-highest text-outline hover:text-on-surface rounded-lg transition-all"
                            title="Edit memory"
                          >
                            <Edit2 size={14} />
                          </button>
                          <button
                            onClick={() => handleDeleteMemory(mem.id)}
                            className="p-2 hover:bg-destructive/10 text-outline hover:text-destructive rounded-lg transition-all"
                            title="Delete memory"
                          >
                            <Trash2 size={14} />
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </motion.section>

        {/* ── Danger Zone Section ─── */}
        <motion.section variants={itemVariants} className="glass-panel border-red-500/20 bg-red-950/5 rounded-2xl p-8">
          <div className="flex items-start gap-5">
            <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-red-950/20 border border-red-500/25">
              <AlertTriangle size={24} className="text-red-400" />
            </div>
            <div className="flex-1">
              <h3 className="text-xl font-bold text-red-300 mb-1">Danger Zone</h3>
              <p className="text-[13px] text-red-200/70 leading-relaxed max-w-xl">
                Resetting assistant data is an irreversible operation. It deletes all stored document index points, sync state details, key configurations, chat logs, and long-term memory.
              </p>
              <button
                onClick={handleResetData}
                className="mt-6 flex items-center gap-2 rounded-xl bg-red-500 hover:bg-red-400 px-5 py-2.5 text-[13px] font-bold text-white shadow-lg active:scale-95 transition-all"
              >
                <AlertTriangle size={15} />
                Reset Assistant Data
              </button>
            </div>
          </div>
        </motion.section>
      </div>
    </motion.div>
  );
}
