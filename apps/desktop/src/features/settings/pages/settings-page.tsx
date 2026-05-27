import { Loader2, Save } from 'lucide-react';
import { useEffect, useState } from 'react';

import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { useGoogleAuthStatusQuery } from '@/features/settings/hooks/use-google-auth-status-query';
import { useSettingsQuery } from '@/features/settings/hooks/use-settings-query';
import { invokeCommand } from '@/lib/api/invoke-command';

export function SettingsPage() {
  const settings = useSettingsQuery();
  const authStatus = useGoogleAuthStatusQuery();
  const [vaultPath, setVaultPath] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'success' | 'error'>('idle');

  // Pre-fill with existing path when data loads
  useEffect(() => {
    if (settings.data?.obsidianVaultPath) {
      setVaultPath(settings.data.obsidianVaultPath);
    }
  }, [settings.data?.obsidianVaultPath]);

  async function handleSaveVaultPath() {
    if (!vaultPath.trim()) return;
    setIsSaving(true);
    setSaveStatus('idle');
    try {
      await invokeCommand('select_obsidian_vault', { path: vaultPath.trim() });
      setSaveStatus('success');
      settings.refetch();
    } catch {
      setSaveStatus('error');
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="grid gap-4 xl:grid-cols-2">
      <Card>
        <h3 className="text-lg font-semibold">Obsidian vault</h3>
        <p className="mt-1 text-sm text-slate-400">
          Current path:{' '}
          <span className="font-mono text-xs text-slate-300">
            {settings.data?.obsidianVaultPath ?? 'Not configured'}
          </span>
        </p>

        <p className="mt-3 text-xs text-slate-500">
          Enter the full path to your Obsidian vault folder (e.g. <code className="text-slate-400">/Users/name/Documents/MyVault</code>)
        </p>

        <input
          className="mt-2 w-full rounded-2xl border border-border/60 bg-white/5 px-4 py-3 font-mono text-sm outline-none focus:border-indigo-500/60 focus:ring-1 focus:ring-indigo-500/30"
          onChange={(event) => {
            setVaultPath(event.target.value);
            setSaveStatus('idle');
          }}
          placeholder="/Users/name/Documents/MyVault"
          value={vaultPath}
        />

        <Button
          className="mt-3 gap-2"
          onClick={handleSaveVaultPath}
          variant="secondary"
          disabled={isSaving || !vaultPath.trim()}
        >
          {isSaving ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Save className="h-4 w-4" />
          )}
          {isSaving ? 'Saving…' : 'Save vault path'}
        </Button>

        {saveStatus === 'success' && (
          <p className="mt-2 text-sm text-emerald-400">✓ Vault path saved — go to Integrations to scan</p>
        )}
        {saveStatus === 'error' && (
          <p className="mt-2 text-sm text-red-400">Failed to save vault path. Check the path is correct.</p>
        )}
      </Card>

      <Card>
        <h3 className="text-lg font-semibold">Google OAuth</h3>
        <p className="mt-2 text-sm text-slate-400">
          Status:{' '}
          <span className={authStatus.data?.connected ? 'text-emerald-400' : 'text-slate-500'}>
            {authStatus.data?.connected ? 'Connected' : 'Not connected'}
          </span>
        </p>
        <p className="mt-1 text-sm text-slate-500">
          Account: {authStatus.data?.email ?? 'No account linked'}
        </p>
      </Card>
    </div>
  );
}
