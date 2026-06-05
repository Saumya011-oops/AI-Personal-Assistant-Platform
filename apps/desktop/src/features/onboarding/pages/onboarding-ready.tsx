import { motion } from 'framer-motion';
import { ArrowRight, Check, Sparkles } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useOnboarding } from '../stores/onboarding-context';

const SOURCE_LABELS: Record<string, string> = {
  notion: 'Notion Workspace',
  drive: 'Google Drive',
  gmail: 'Gmail Inbox',
  gcal: 'Google Calendar',
  'apple-cal': 'Apple Calendar',
  obsidian: 'Obsidian Vault',
  local: 'Local Files',
};

const AI_LABELS: Record<string, string> = {
  groq: 'Groq Cloud Inference',
  ollama: 'Ollama (Fully Local)',
  hybrid: 'Hybrid Mode (Auto-Switch)',
};

export function ReadyStep() {
  const { sources, ai } = useOnboarding();
  const navigate = useNavigate();

  const handleComplete = () => {
    localStorage.setItem('onboarding_complete', 'true');
    // Force a page refresh or route redirection to trigger ProtectedRoute updates
    navigate('/', { replace: true });
    window.location.reload();
  };

  return (
    <div className="text-center">
      <motion.div
        initial={{ scale: 0.4, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: 'spring', stiffness: 180, damping: 16 }}
        className="relative mx-auto mt-4 flex h-20 w-20 items-center justify-center"
      >
        <div className="absolute inset-0 -m-3 rounded-full bg-primary-glass/30 blur-2xl animate-pulse-glow" />
        <div className="relative flex h-20 w-20 items-center justify-center rounded-full bg-primary-glass text-black glow">
          <Check className="h-8 w-8 stroke-[3]" />
        </div>
      </motion.div>

      <motion.h1
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.2 }}
        className="mt-8 text-3xl font-bold text-gradient md:text-4xl"
      >
        Your assistant is ready.
      </motion.h1>
      <motion.p
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.3 }}
        className="mx-auto mt-3 max-w-md text-sm text-outline md:text-base leading-relaxed"
      >
        Lumen has indexed your knowledge and is running locally. Ask anything — it will retrieve relevant context and cite sources automatically.
      </motion.p>

      <motion.div
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.4 }}
        className="mx-auto mt-10 max-w-lg rounded-2xl glass-strong p-6 text-left border border-primary-glass/10"
      >
        <div className="text-[10px] font-mono uppercase tracking-widest text-outline">Configuration Summary</div>
        <div className="mt-4 grid gap-4 sm:grid-cols-2">
          <div>
            <div className="text-xs text-outline font-semibold mb-1.5">Connected Sources</div>
            <div className="flex flex-wrap gap-1.5">
              {sources.length === 0 ? (
                <span className="text-xs text-outline italic">None</span>
              ) : (
                sources.map((s) => (
                  <span
                    key={s}
                    className="rounded-full bg-primary-glass/10 px-2.5 py-0.5 text-xs text-primary-glass font-medium border border-primary-glass/25"
                  >
                    {SOURCE_LABELS[s] || s}
                  </span>
                ))
              )}
            </div>
          </div>
          <div>
            <div className="text-xs text-outline font-semibold mb-1.5">AI Engine Setup</div>
            <div className="inline-flex items-center gap-1.5 rounded-full bg-surface-container-high px-3 py-1 text-xs text-white border border-outline-variant/30">
              <Sparkles className="h-3.5 w-3.5 text-primary-glass" />
              {AI_LABELS[ai] || ai}
            </div>
          </div>
        </div>
      </motion.div>

      <motion.div
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.5 }}
        className="mt-10"
      >
        <button
          type="button"
          onClick={handleComplete}
          className="group inline-flex items-center gap-2 rounded-full bg-primary-glass px-6 py-3 text-sm font-bold text-black transition-all hover:glow"
        >
          Open Workspace
          <ArrowRight className="h-4 w-4 transition-transform group-hover:translate-x-0.5" />
        </button>
      </motion.div>
    </div>
  );
}
