import { Outlet, useMatches, useLocation } from 'react-router-dom';
import { AlignJustify, Search, WifiOff, Zap } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

import { AppSidebar } from '@/components/shell/app-sidebar';
import { CommandPalette } from '@/components/shell/command-palette';
import { useUiStore } from '@/stores/ui-store';

export function AppShell() {
  const openPalette = useUiStore((state) => state.openCommandPalette);
  const matches = useMatches();
  const location = useLocation();
  const activeHandle = (matches.at(-1)?.handle ?? {
    title: 'Home',
    subtitle: 'AI PERSONAL ASSISTANT PLATFORM',
  }) as { title: string; subtitle: string };

  return (
    <div className="flex h-screen overflow-hidden bg-[#0b1326] text-on-surface relative">
      {/* Background grid pattern (adds high aesthetic value) */}
      <div className="pointer-events-none absolute inset-0 grid-bg opacity-30 z-0" />

      <AppSidebar />

      <div className="flex flex-1 flex-col overflow-hidden min-w-0 z-10 bg-transparent">
        {/* ── Top Header ─────────────────────────────────── */}
        <header className="flex h-14 shrink-0 items-center justify-between border-b border-surface-container-highest/40 bg-[#0b1326]/40 backdrop-blur-md px-6 z-40">
          {/* Left: page info */}
          <div className="flex items-center gap-4">
            <div>
              <p className="font-mono text-[10px] uppercase tracking-widest text-outline leading-none">
                {activeHandle.subtitle ?? 'AI PERSONAL ASSISTANT PLATFORM'}
              </p>
              <h1 className="text-[15px] font-semibold text-on-surface leading-tight mt-0.5">
                {activeHandle.title}
              </h1>
            </div>
          </div>

          {/* Right: status pills */}
          <div className="flex items-center gap-2">
            {/* Offline-first pill */}
            <div className="flex items-center gap-1.5 rounded-full border border-surface-container-highest/50 bg-surface-container-high/20 px-3 py-1.5">
              <WifiOff size={12} className="text-outline" />
              <span className="font-mono text-[11px] text-outline">Offline-first</span>
            </div>

            {/* Retrieval Ready pill */}
            <div className="flex items-center gap-1.5 rounded-full border border-tertiary/20 bg-tertiary/10 px-3 py-1.5">
              <span className="h-1.5 w-1.5 rounded-full bg-tertiary animate-ai-pulse" />
              <span className="font-mono text-[11px] text-tertiary font-medium">Retrieval ready</span>
            </div>
          </div>
        </header>

        {/* ── Page Content ───────────────────────────────── */}
        <main className="flex-1 overflow-auto custom-scrollbar bg-transparent">
          <div className="min-h-full p-6">
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={location.pathname}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -10 }}
                transition={{ duration: 0.2, ease: 'easeOut' }}
              >
                <Outlet />
              </motion.div>
            </AnimatePresence>
          </div>
        </main>

        {/* ── Footer ─────────────────────────────────────── */}
        <footer className="flex shrink-0 items-center justify-between border-t border-surface-container-highest/40 bg-[#0b1326]/40 backdrop-blur-md px-6 py-2.5 z-40">
          <div className="flex items-center gap-6">
            <p className="font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-tertiary">
              © 2025 Assistant Core. All systems operational.
            </p>
            <div className="flex gap-4">
              {['Documentation', 'Local API', 'Privacy', 'Support'].map((link) => (
                <a
                  key={link}
                  className="font-mono text-[10px] text-outline hover:text-on-surface transition-colors"
                  href="#"
                  onClick={(e) => e.preventDefault()}
                >
                  {link}
                </a>
              ))}
            </div>
          </div>

          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-green-500 shadow-[0_0_6px_rgba(34,197,94,0.6)]" />
              <span className="font-mono text-[10px] font-medium text-on-surface">Local Node Syncing</span>
            </div>
          </div>
        </footer>
      </div>

      <CommandPalette />
    </div>
  );
}
