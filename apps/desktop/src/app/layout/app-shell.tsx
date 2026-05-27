import { Command } from 'lucide-react';
import { Outlet } from 'react-router-dom';

import { AppSidebar } from '@/components/shell/app-sidebar';
import { CommandPalette } from '@/components/shell/command-palette';
import { Button } from '@/components/ui/button';
import { useUiStore } from '@/stores/ui-store';

export function AppShell() {
  const openPalette = useUiStore((state) => state.openCommandPalette);

  return (
    <div className="flex min-h-screen bg-background text-foreground">
      <AppSidebar />
      <div className="flex flex-1 flex-col">
        <header className="flex h-16 items-center justify-between border-b border-border/60 px-6 backdrop-blur">
          <div>
            <p className="text-xs uppercase tracking-[0.28em] text-slate-400">
              Desktop AI Workspace
            </p>
            <h1 className="text-lg font-semibold">AI Personal Assistant</h1>
          </div>
          <Button
            className="gap-2"
            variant="secondary"
            onClick={openPalette}
          >
            <Command className="h-4 w-4" />
            Command Palette
          </Button>
        </header>
        <main className="flex-1 overflow-auto p-6">
          <Outlet />
        </main>
      </div>
      <CommandPalette />
    </div>
  );
}
