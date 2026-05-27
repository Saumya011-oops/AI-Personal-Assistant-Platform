import {
  Database,
  FolderSync,
  LayoutDashboard,
  Settings,
  Sparkles,
} from 'lucide-react';
import { NavLink } from 'react-router-dom';

import { cn } from '@/lib/utils/cn';
import { useUiStore } from '@/stores/ui-store';

const items = [
  { href: '/', label: 'Workspace', icon: LayoutDashboard },
  { href: '/documents', label: 'Documents', icon: Database },
  { href: '/integrations', label: 'Integrations', icon: FolderSync },
  { href: '/settings', label: 'Settings', icon: Settings },
];

export function AppSidebar() {
  const collapsed = useUiStore((state) => state.sidebarCollapsed);
  const toggle = useUiStore((state) => state.toggleSidebar);

  return (
    <aside
      className={cn(
        'border-r border-border/60 bg-[#0a0d12] transition-all duration-200',
        collapsed ? 'w-[88px]' : 'w-[280px]',
      )}
    >
      <div className="flex h-16 items-center justify-between px-5">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-slate-100/10">
            <Sparkles className="h-5 w-5 text-sky-300" />
          </div>
          {!collapsed && (
            <div>
              <div className="text-sm font-semibold">Assistant Core</div>
              <div className="text-xs text-slate-400">Foundation phase</div>
            </div>
          )}
        </div>
        <button
          className="text-xs text-slate-400"
          onClick={toggle}
          type="button"
        >
          {collapsed ? 'Expand' : 'Collapse'}
        </button>
      </div>
      <nav className="space-y-2 px-3 py-4">
        {items.map(({ href, label, icon: Icon }) => (
          <NavLink
            key={href}
            className={({ isActive }) =>
              cn(
                'flex items-center gap-3 rounded-2xl px-4 py-3 text-sm transition-colors',
                isActive
                  ? 'bg-sky-500/15 text-sky-200'
                  : 'text-slate-300 hover:bg-white/5 hover:text-white',
              )
            }
            to={href}
          >
            <Icon className="h-4 w-4 shrink-0" />
            {!collapsed && <span>{label}</span>}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
