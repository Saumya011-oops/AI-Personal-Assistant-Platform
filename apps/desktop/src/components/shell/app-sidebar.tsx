import { NavLink, useLocation } from 'react-router-dom';
import { motion } from 'framer-motion';
import {
  Home,
  MessageSquare,
  Database,
  RefreshCw,
  Settings,
  Search,
  ChevronRight,
  MoreHorizontal,
  Zap,
  LogOut,
} from 'lucide-react';
import { cn } from '@/lib/utils/cn';
import { useUiStore } from '@/stores/ui-store';
import { invokeCommand } from '@/lib/api/invoke-command';

interface NavItem {
  href: string;
  label: string;
  sublabel: string;
  icon: React.ReactNode;
  section: 'workspace' | 'system';
}

const navItems: NavItem[] = [
  {
    href: '/',
    label: 'Home',
    sublabel: 'Workspace overview',
    icon: <Home size={18} />,
    section: 'workspace',
  },
  {
    href: '/assistant',
    label: 'Assistant',
    sublabel: 'Grounded chat and citations',
    icon: <MessageSquare size={18} />,
    section: 'workspace',
  },
  {
    href: '/documents',
    label: 'Knowledge Base',
    sublabel: 'Split-view document explorer',
    icon: <Database size={18} />,
    section: 'workspace',
  },
  {
    href: '/integrations',
    label: 'Integrations',
    sublabel: 'Sources and sync',
    icon: <RefreshCw size={18} />,
    section: 'system',
  },
  {
    href: '/settings',
    label: 'Settings',
    sublabel: 'Vaults, auth, preferences',
    icon: <Settings size={18} />,
    section: 'system',
  },
];

export function AppSidebar() {
  const location = useLocation();
  const openPalette = useUiStore((state) => state.openCommandPalette);

  const handleLogout = async () => {
    const confirmLogout = window.confirm("Are you sure you want to log out? All your saved credentials and local data will be deleted.");
    if (!confirmLogout) return;
    try {
      await invokeCommand('logout_and_reset', {});
      localStorage.removeItem('onboarding_complete');
      window.location.href = '/onboarding';
    } catch (err) {
      console.error('Logout failed:', err);
      alert('Failed to delete all data during logout.');
    }
  };

  const workspaceItems = navItems.filter((i) => i.section === 'workspace');
  const systemItems = navItems.filter((i) => i.section === 'system');

  return (
    <aside className="flex h-screen w-[260px] shrink-0 flex-col border-r border-surface-container-highest/40 bg-[#0b1326]/60 backdrop-blur-xl z-50 shadow-2xl">
      {/* ── Logo ─────────────────────────────────────── */}
      <div className="flex items-center gap-3 p-5 pb-4">
        <motion.div 
          whileHover={{ scale: 1.05, rotate: 5 }}
          whileTap={{ scale: 0.95 }}
          className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary-glass/10 text-primary-glass glow-sm cursor-pointer"
        >
          <Zap size={18} fill="currentColor" />
        </motion.div>
        <div>
          <h1 className="text-[16px] font-bold text-on-surface tracking-tight leading-tight">
            Assistant Core
          </h1>
          <p className="text-[11px] text-outline">Local-first workspace</p>
        </div>
      </div>

      {/* ── Command Search ────────────────────────────── */}
      <div className="px-4 pb-5">
        <button
          className="flex w-full items-center justify-between rounded-xl border border-outline-variant/20 bg-surface-container-high/40 px-3 py-2.5 text-on-surface-variant transition-all hover:bg-surface-container-high hover:border-outline-variant/40 group"
          onClick={openPalette}
          type="button"
          id="sidebar-search-btn"
        >
          <div className="flex items-center gap-2">
            <Search size={15} />
            <span className="text-[13px]">Command Search</span>
          </div>
          <span className="font-mono text-[10px] rounded border border-outline-variant/30 bg-surface-container-highest px-1.5 py-0.5">
            ⌘ K
          </span>
        </button>
      </div>

      {/* ── Navigation ───────────────────────────────── */}
      <nav className="flex-1 px-3 space-y-5 overflow-auto custom-scrollbar">
        {/* Workspace section */}
        <div>
          <p className="px-3 mb-2 text-[10px] font-bold uppercase tracking-widest text-outline">
            Workspace
          </p>
          <div className="space-y-0.5">
            {workspaceItems.map((item) => {
              const isActive =
                item.href === '/'
                  ? location.pathname === '/'
                  : location.pathname.startsWith(item.href);

              return (
                <NavLink
                  key={item.href}
                  to={item.href}
                  end={item.href === '/'}
                  className={cn(
                    'relative flex items-center gap-3 rounded-xl px-3 py-2.5 transition-all group overflow-hidden',
                    isActive
                      ? 'text-primary-glass font-semibold'
                      : 'text-on-surface-variant hover:text-on-surface',
                  )}
                >
                  {isActive && (
                    <motion.div
                      layoutId="active-sidebar-nav"
                      className="absolute inset-0 bg-primary-glass/8 border-l-2 border-primary-glass"
                      transition={{ type: 'spring', stiffness: 380, damping: 30 }}
                    />
                  )}
                  <motion.span 
                    whileHover={{ x: 3 }}
                    transition={{ type: 'spring', stiffness: 400, damping: 20 }}
                    className="relative z-10 flex items-center gap-3 w-full"
                  >
                    <span className={cn('shrink-0', isActive ? 'text-primary-glass' : 'group-hover:text-on-surface transition-colors')}>
                      {item.icon}
                    </span>
                    <span className="text-[14px] font-medium">{item.label}</span>
                  </motion.span>
                </NavLink>
              );
            })}
          </div>
        </div>

        {/* System section */}
        <div>
          <p className="px-3 mb-2 text-[10px] font-bold uppercase tracking-widest text-outline">
            System
          </p>
          <div className="space-y-0.5">
            {systemItems.map((item) => {
              const isActive = location.pathname.startsWith(item.href);

              return (
                <NavLink
                  key={item.href}
                  to={item.href}
                  className={cn(
                    'relative flex items-center gap-3 rounded-xl px-3 py-2.5 transition-all group overflow-hidden',
                    isActive
                      ? 'text-primary-glass font-semibold'
                      : 'text-on-surface-variant hover:text-on-surface',
                  )}
                >
                  {isActive && (
                    <motion.div
                      layoutId="active-sidebar-nav"
                      className="absolute inset-0 bg-primary-glass/8 border-l-2 border-primary-glass"
                      transition={{ type: 'spring', stiffness: 380, damping: 30 }}
                    />
                  )}
                  <motion.span 
                    whileHover={{ x: 3 }}
                    transition={{ type: 'spring', stiffness: 400, damping: 20 }}
                    className="relative z-10 flex items-center gap-3 w-full"
                  >
                    <span className={cn('shrink-0', isActive ? 'text-primary-glass' : 'group-hover:text-on-surface transition-colors')}>
                      {item.icon}
                    </span>
                    <span className="text-[14px] font-medium">{item.label}</span>
                  </motion.span>
                </NavLink>
              );
            })}
          </div>
        </div>
      </nav>

      {/* ── User Card ────────────────────────────────── */}
      <div className="mt-auto border-t border-surface-container-highest/40 p-4">
        <div className="flex items-center gap-3 rounded-xl px-2 py-2">
          <div className="h-8 w-8 shrink-0 rounded-full border border-outline-variant bg-surface-container-highest flex items-center justify-center overflow-hidden">
            <span className="text-[11px] font-bold text-primary-glass">ST</span>
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-[13px] font-medium text-on-surface truncate">Saumya Thacker</p>
            <p className="text-[11px] text-outline">Workspace owner</p>
          </div>
          <button
            onClick={handleLogout}
            className="text-outline hover:text-red-400 transition-colors p-1.5 rounded-lg hover:bg-red-500/10"
            type="button"
            title="Log out and reset all data"
            aria-label="Log out"
          >
            <LogOut size={16} />
          </button>
        </div>
      </div>
    </aside>
  );
}
