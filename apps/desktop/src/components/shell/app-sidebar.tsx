import {
  BellDot,
  Bot,
  ChevronRight,
  CreditCard,
  FileSearch,
  FolderSync,
  Home,
  Search,
  Settings,
  Sparkles,
} from 'lucide-react';
import type { ComponentType } from 'react';
import { NavLink, useLocation } from 'react-router-dom';

import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import { Badge } from '@/components/ui/badge';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { cn } from '@/lib/utils/cn';
import { useUiStore } from '@/stores/ui-store';

const primaryItems = [
  { href: '/', label: 'Home', icon: Home, description: 'Workspace overview' },
  { href: '/assistant', label: 'Assistant', icon: Bot, description: 'Grounded chat and citations' },
  {
    href: '/documents',
    label: 'Knowledge Base',
    icon: FileSearch,
    description: 'Split-view document explorer',
  },
];

const systemItems = [
  {
    href: '/integrations',
    label: 'Integrations',
    icon: FolderSync,
    description: 'Sources and sync',
  },
  {
    href: '/settings',
    label: 'Settings',
    icon: Settings,
    description: 'Vaults, auth, preferences',
  },
];

const pinnedSignals = [
  { label: 'Offline-first', tone: 'success' as const },
  { label: 'RAG Core', tone: 'secondary' as const },
  { label: 'Desktop', tone: 'outline' as const },
];

export function AppSidebar() {
  const location = useLocation();
  const collapsed = useUiStore((state) => state.sidebarCollapsed);
  const toggle = useUiStore((state) => state.toggleSidebar);

  return (
    <aside
      className={cn(
        'flex h-screen flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground transition-all duration-200',
        collapsed ? 'w-[88px]' : 'w-[290px]',
      )}
    >
      <div className="flex h-16 items-center justify-between px-4">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-sidebar-primary text-sidebar-primary-foreground">
            <Sparkles className="h-5 w-5" />
          </div>
          {!collapsed && (
            <div>
              <div className="text-sm font-semibold">Assistant Core</div>
              <div className="text-xs text-muted-foreground">Local-first workspace</div>
            </div>
          )}
        </div>
        <button
          className="text-xs text-muted-foreground transition hover:text-foreground"
          onClick={toggle}
          type="button"
        >
          {collapsed ? 'Expand' : 'Collapse'}
        </button>
      </div>

      <div className="px-4 pb-3">
        <div className="rounded-3xl border border-sidebar-border bg-sidebar-accent/70 p-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Search className="h-4 w-4 text-primary" />
              {!collapsed && <span className="text-sm font-medium">Command Search</span>}
            </div>
            {!collapsed && <Badge variant="outline">Cmd K</Badge>}
          </div>
          {!collapsed && (
            <p className="mt-2 text-xs leading-relaxed text-muted-foreground">
              Jump to chat, sources, citations, and settings without leaving the keyboard.
            </p>
          )}
        </div>
      </div>

      <nav className="flex-1 space-y-4 overflow-auto px-3 pb-3">
        <SidebarSection collapsed={collapsed} items={primaryItems} title="Workspace" />

        <Collapsible defaultOpen>
          <div className="rounded-3xl border border-sidebar-border bg-sidebar-accent/50 px-2 py-2">
            <CollapsibleTrigger className="flex w-full items-center justify-between rounded-2xl px-3 py-2 text-left text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">
              {!collapsed ? 'System' : 'Sys'}
              {!collapsed && <ChevronRight className="h-3.5 w-3.5 transition group-data-[state=open]:rotate-90" />}
            </CollapsibleTrigger>
            <CollapsibleContent className="space-y-1 pb-1">
              {systemItems.map((item) => (
                <SidebarLink key={item.href} collapsed={collapsed} {...item} />
              ))}
            </CollapsibleContent>
          </div>
        </Collapsible>

        {!collapsed && (
          <div className="rounded-3xl border border-sidebar-border bg-sidebar-accent/40 p-3">
            <div className="flex items-center gap-2 text-sm font-medium">
              <BellDot className="h-4 w-4 text-primary" />
              Productivity signals
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              {pinnedSignals.map((signal) => (
                <Badge key={signal.label} variant={signal.tone}>
                  {signal.label}
                </Badge>
              ))}
            </div>
            <div className="mt-3 rounded-2xl border border-sidebar-border bg-background/40 p-3 text-xs leading-relaxed text-muted-foreground">
              {location.pathname === '/'
                ? 'Landing stays calm and focused on readiness, actions, and source health.'
                : location.pathname === '/assistant'
                  ? 'Assistant mode keeps chat, evidence, and retrieval context in one focused view.'
                  : 'The side rail stays stable across documents, integrations, and settings.'}
            </div>
          </div>
        )}
      </nav>

      <div className="border-t border-sidebar-border p-3">
        <div className="flex items-center gap-3 rounded-2xl bg-sidebar-accent px-3 py-3">
          <Avatar className="h-9 w-9">
            <AvatarFallback>ST</AvatarFallback>
          </Avatar>
          {!collapsed && (
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium">Saumya Thacker</div>
              <div className="truncate text-xs text-muted-foreground">Workspace owner</div>
            </div>
          )}
          {!collapsed && <CreditCard className="h-4 w-4 text-muted-foreground" />}
        </div>
      </div>
    </aside>
  );
}

function SidebarSection({
  title,
  items,
  collapsed,
}: {
  title: string;
  items: Array<{
    href: string;
    label: string;
    description: string;
    icon: ComponentType<{ className?: string }>;
  }>;
  collapsed: boolean;
}) {
  return (
    <div className="rounded-3xl border border-sidebar-border bg-sidebar-accent/50 px-2 py-2">
      {!collapsed && (
        <div className="px-3 py-2 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">
          {title}
        </div>
      )}
      <div className="space-y-1">
        {items.map((item) => (
          <SidebarLink key={item.href} collapsed={collapsed} {...item} />
        ))}
      </div>
    </div>
  );
}

function SidebarLink({
  href,
  label,
  description,
  icon: Icon,
  collapsed,
}: {
  href: string;
  label: string;
  description: string;
  icon: ComponentType<{ className?: string }>;
  collapsed: boolean;
}) {
  return (
    <NavLink
      className={({ isActive }) =>
        cn(
          'group flex items-center gap-3 rounded-2xl px-3 py-2.5 transition-colors',
          isActive
            ? 'bg-sidebar-primary/12 text-foreground'
            : 'text-sidebar-foreground/80 hover:bg-sidebar-accent hover:text-foreground',
        )
      }
      to={href}
    >
      {({ isActive }) => (
        <>
          <div
            className={cn(
              'flex h-8 w-8 shrink-0 items-center justify-center rounded-xl border border-transparent bg-background/10',
              isActive && 'border-primary/20 bg-primary/16 text-primary',
            )}
          >
            <Icon className="h-4 w-4" />
          </div>
          {!collapsed && (
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium">{label}</div>
              <div className="truncate text-xs text-muted-foreground">{description}</div>
            </div>
          )}
        </>
      )}
    </NavLink>
  );
}
