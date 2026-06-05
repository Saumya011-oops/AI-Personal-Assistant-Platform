import { Link, Outlet, useLocation } from 'react-router-dom';
import { Sparkles } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { OnboardingProvider } from '../stores/onboarding-context';

const steps = [
  { path: '/onboarding/sources', label: 'Sources' },
  { path: '/onboarding/ai-setup', label: 'AI setup' },
  { path: '/onboarding/indexing', label: 'Indexing' },
  { path: '/onboarding/ready', label: 'Ready' },
];

export function OnboardingLayout() {
  const { pathname } = useLocation();
  const isWelcome = pathname === '/onboarding' || pathname === '/onboarding/';
  const currentIdx = Math.max(
    0,
    steps.findIndex((s) => pathname.startsWith(s.path)),
  );

  return (
    <OnboardingProvider>
      <div className="relative h-screen overflow-y-auto bg-[#0b1326] text-on-surface custom-scrollbar">
        <div className="pointer-events-none absolute inset-0 grid-bg" />

        <header className={`relative mx-auto flex items-center justify-between px-6 pt-8 z-10 ${isWelcome ? 'max-w-5xl' : 'max-w-3xl'}`}>
          <div className="flex items-center gap-2 font-semibold select-none">
            <span className="inline-flex h-8 w-8 items-center justify-center rounded-lg bg-primary-glass/10 text-primary-glass glow-sm">
              <Sparkles className="h-4 w-4" />
            </span>
            <span className="text-sm font-bold tracking-tight text-white">Lumen</span>
          </div>
          {!isWelcome && (
            <div className="text-xs text-outline font-mono">
              Step <span className="text-primary-glass font-bold">{currentIdx + 1}</span> of {steps.length}
            </div>
          )}
        </header>

        {/* Progress bar */}
        {!isWelcome && (
          <div className="relative mx-auto mt-6 max-w-3xl px-6 z-10">
            <div className="flex items-center gap-3">
              {steps.map((s, i) => (
                <div key={s.path} className="flex flex-1 flex-col gap-2">
                  <div
                    className={`h-1 rounded-full transition-all duration-300 ${
                      i <= currentIdx ? 'bg-primary-glass' : 'bg-surface-container-highest'
                    }`}
                  />
                  <div
                    className={`text-[10px] font-mono uppercase tracking-wider ${
                      i === currentIdx ? 'text-primary-glass font-bold' : 'text-outline'
                    }`}
                  >
                    {s.label}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        <main className={`relative mx-auto px-6 pb-16 pt-10 z-10 ${isWelcome ? 'max-w-5xl' : 'max-w-3xl'}`}>
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={pathname}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              transition={{ duration: 0.2, ease: 'easeOut' }}
            >
              <Outlet />
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
    </OnboardingProvider>
  );
}

