import { CheckCircle2, Loader2, LockKeyhole, Save, Settings2 } from 'lucide-react';
import { useEffect, useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useGoogleAuthStatusQuery } from '@/features/settings/hooks/use-google-auth-status-query';
import { useSettingsQuery } from '@/features/settings/hooks/use-settings-query';
import { invokeCommand } from '@/lib/api/invoke-command';

export function SettingsPage() {
  const settings = useSettingsQuery();
  const authStatus = useGoogleAuthStatusQuery();
  const [vaultPath, setVaultPath] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'success' | 'error'>('idle');

  useEffect(() => {
    if (settings.data?.obsidianVaultPath) {
      setVaultPath(settings.data.obsidianVaultPath);
    }
  }, [settings.data?.obsidianVaultPath]);

  async function handleSaveVaultPath() {
    if (!vaultPath.trim()) {
      return;
    }

    setIsSaving(true);
    setSaveStatus('idle');
    try {
      await invokeCommand('select_obsidian_vault', { path: vaultPath.trim() });
      setSaveStatus('success');
      await settings.refetch();
    } catch {
      setSaveStatus('error');
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="space-y-4">
      <Card className="p-5">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
              Control Room
            </p>
            <h2 className="mt-1 text-xl font-semibold">Settings and authentication</h2>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant="outline">Desktop preferences</Badge>
            <Badge variant="secondary">Secure credentials</Badge>
          </div>
        </div>
      </Card>

      <Tabs defaultValue="sources">
        <TabsList>
          <TabsTrigger value="sources">Sources</TabsTrigger>
          <TabsTrigger value="auth">Authentication</TabsTrigger>
          <TabsTrigger value="workspace">Workspace</TabsTrigger>
        </TabsList>

        <TabsContent value="sources">
          <Card className="p-5">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-secondary">
                <Settings2 className="h-5 w-5 text-primary" />
              </div>
              <div>
                <h3 className="text-lg font-semibold">Obsidian vault path</h3>
                <p className="mt-1 text-sm text-muted-foreground">
                  Point the desktop app at the local knowledge vault you want indexed.
                </p>
              </div>
            </div>

            <div className="mt-5 space-y-3">
              <div>
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Current path
                </p>
                <p className="mt-2 rounded-2xl border border-border bg-secondary/55 px-4 py-3 font-mono text-sm">
                  {settings.data?.obsidianVaultPath ?? 'Not configured'}
                </p>
              </div>
              <div>
                <p className="mb-2 text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  New vault path
                </p>
                <Input
                  onChange={(event) => {
                    setVaultPath(event.target.value);
                    setSaveStatus('idle');
                  }}
                  placeholder="/Users/name/Documents/MyVault"
                  value={vaultPath}
                />
              </div>
              <Button
                className="gap-2"
                disabled={isSaving || !vaultPath.trim()}
                onClick={handleSaveVaultPath}
                variant="secondary"
              >
                {isSaving ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Save className="h-4 w-4" />
                )}
                {isSaving ? 'Saving path' : 'Save vault path'}
              </Button>
              {saveStatus === 'success' ? (
                <p className="text-sm text-emerald-400">
                  Vault path saved. The integration panel can now scan it.
                </p>
              ) : null}
              {saveStatus === 'error' ? (
                <p className="text-sm text-red-400">
                  Saving failed. Verify the path and try again.
                </p>
              ) : null}
            </div>
          </Card>
        </TabsContent>

        <TabsContent value="auth">
          <div className="grid gap-4 xl:grid-cols-2">
            <Card className="p-5">
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-secondary">
                  <LockKeyhole className="h-5 w-5 text-primary" />
                </div>
                <div>
                  <h3 className="text-lg font-semibold">Google OAuth</h3>
                  <p className="mt-1 text-sm text-muted-foreground">
                    Foundation for Gmail and Calendar extensions.
                  </p>
                </div>
              </div>
              <div className="mt-5 space-y-3">
                <div className="flex items-center justify-between rounded-2xl border border-border bg-secondary/50 px-4 py-3">
                  <span className="text-sm">Status</span>
                  <Badge variant={authStatus.data?.connected ? 'success' : 'outline'}>
                    {authStatus.data?.connected ? 'Connected' : 'Not connected'}
                  </Badge>
                </div>
                <div className="rounded-2xl border border-border bg-secondary/50 px-4 py-3">
                  <p className="text-sm">Account</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {authStatus.data?.email ?? 'No Google account linked yet'}
                  </p>
                </div>
              </div>
            </Card>

            <Card className="p-5">
              <h3 className="text-lg font-semibold">Credential posture</h3>
              <div className="mt-4 space-y-3">
                {[
                  'Tokens are stored via the backend credential service.',
                  'PKCE is supported in the OAuth foundation.',
                  'Desktop settings remain separate from integration credentials.',
                ].map((item) => (
                  <div
                    key={item}
                    className="flex items-start gap-3 rounded-2xl border border-border bg-secondary/55 px-4 py-3"
                  >
                    <CheckCircle2 className="mt-0.5 h-4 w-4 text-emerald-400" />
                    <p className="text-sm text-muted-foreground">{item}</p>
                  </div>
                ))}
              </div>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="workspace">
          <Card className="p-5">
            <h3 className="text-lg font-semibold">Workspace defaults</h3>
            <div className="mt-4 grid gap-3 md:grid-cols-3">
              {[
                'Dark theme only for the desktop foundation phase.',
                'Command palette remains the primary navigation accelerator.',
                'Layouts favor dense horizontal work over stacked mobile patterns.',
              ].map((item) => (
                <div
                  key={item}
                  className="rounded-2xl border border-border bg-secondary/55 px-4 py-4 text-sm text-muted-foreground"
                >
                  {item}
                </div>
              ))}
            </div>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
