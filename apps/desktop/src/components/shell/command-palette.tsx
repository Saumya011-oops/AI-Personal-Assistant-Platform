import { Bot, Command, Database, FolderSync, Home, Settings, Sparkles } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import {
  Command as CommandMenu,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from '@/components/ui/command';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { useUiStore } from '@/stores/ui-store';

export function CommandPalette() {
  const navigate = useNavigate();
  const isOpen = useUiStore((state) => state.commandPaletteOpen);
  const close = useUiStore((state) => state.closeCommandPalette);
  const [query, setQuery] = useState('');

  const actions = useMemo(
    () => [
      {
        label: 'Open workspace home',
        detail: 'Overview, readiness, and quick actions',
        shortcut: '1',
        icon: Home,
        run: () => navigate('/'),
      },
      {
        label: 'Open assistant workspace',
        detail: 'Chat, citations, and retrieval',
        shortcut: '2',
        icon: Bot,
        run: () => navigate('/assistant'),
      },
      {
        label: 'Browse knowledge base',
        detail: 'Inspect indexed documents',
        shortcut: '3',
        icon: Database,
        run: () => navigate('/documents'),
      },
      {
        label: 'Manage integrations',
        detail: 'Sources, sync, and auth',
        shortcut: '4',
        icon: FolderSync,
        run: () => navigate('/integrations'),
      },
      {
        label: 'Open settings',
        detail: 'Vaults, preferences, and auth state',
        shortcut: '5',
        icon: Settings,
        run: () => navigate('/settings'),
      },
    ],
    [navigate],
  );

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) {
          setQuery('');
          close();
        }
      }}
    >
      <DialogContent className="max-w-2xl p-0">
        <CommandMenu shouldFilter value={query} onValueChange={setQuery}>
          <CommandInput placeholder="Search actions, pages, and workflows…" />
          <CommandList className="max-h-[420px] overflow-auto px-2 pb-3">
            <CommandEmpty>No matching action.</CommandEmpty>
            <CommandGroup heading="Workspace">
              {actions.map((action) => (
                <CommandItem
                  key={action.label}
                  onSelect={() => {
                    action.run();
                    setQuery('');
                    close();
                  }}
                >
                  <div className="flex h-9 w-9 items-center justify-center rounded-xl border border-border bg-secondary">
                    <action.icon className="h-4 w-4" />
                  </div>
                  <div className="min-w-0">
                    <div className="text-sm font-medium">{action.label}</div>
                    <div className="truncate text-xs text-muted-foreground">
                      {action.detail}
                    </div>
                  </div>
                  <CommandShortcut>{action.shortcut}</CommandShortcut>
                </CommandItem>
              ))}
            </CommandGroup>
            <CommandGroup heading="Suggestions">
              <CommandItem>
                <Sparkles className="h-4 w-4" />
                Ask the assistant about a synced document
                <CommandShortcut>AI</CommandShortcut>
              </CommandItem>
              <CommandItem>
                <Command className="h-4 w-4" />
                Open keyboard shortcuts
                <CommandShortcut>?</CommandShortcut>
              </CommandItem>
            </CommandGroup>
          </CommandList>
        </CommandMenu>
      </DialogContent>
    </Dialog>
  );
}
