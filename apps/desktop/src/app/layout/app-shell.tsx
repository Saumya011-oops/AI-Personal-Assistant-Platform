import { Command, PanelLeftClose, SearchCheck, Wifi } from 'lucide-react';
import { Outlet, useMatches } from 'react-router-dom';

import { AppSidebar } from '@/components/shell/app-sidebar';
import { CommandPalette } from '@/components/shell/command-palette';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { useUiStore } from '@/stores/ui-store';

export function AppShell() {
  const openPalette = useUiStore((state) => state.openCommandPalette);
  const toggleSidebar = useUiStore((state) => state.toggleSidebar);
  const matches = useMatches();
  const activeHandle = (matches.at(-1)?.handle ?? {
    title: 'Home',
    subtitle: 'Workspace overview',
  }) as { title: string; subtitle: string };

  return (
    <div className="flex min-h-screen bg-background text-foreground">
      <AppSidebar />
      <div className="flex flex-1 flex-col">
        <header className="sticky top-0 z-20 flex h-16 items-center justify-between border-b border-border/80 bg-background/86 px-5 backdrop-blur">
          <div className="flex items-center gap-4">
            <Button className="text-muted-foreground" onClick={toggleSidebar} size="icon" variant="ghost">
              <PanelLeftClose className="h-4 w-4" />
            </Button>
            <Separator className="h-5" orientation="vertical" />
            <div>
              <p className="text-[11px] uppercase tracking-[0.24em] text-muted-foreground">
                AI Personal Assistant Platform
              </p>
              <h1 className="text-base font-semibold">{activeHandle.title}</h1>
              <p className="text-xs text-muted-foreground">{activeHandle.subtitle}</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <Badge variant="outline">
              <Wifi className="mr-1.5 h-3 w-3" />
              Offline-first
            </Badge>
            <Badge variant="success">
              <SearchCheck className="mr-1.5 h-3 w-3" />
              Retrieval ready
            </Badge>
            <Button className="gap-2" variant="secondary" onClick={openPalette}>
              <Command className="h-4 w-4" />
              Search
            </Button>
          </div>
        </header>
        <main className="flex-1 overflow-auto p-5">
          <Outlet />
        </main>
      </div>
      <CommandPalette />
    </div>
  );
}
