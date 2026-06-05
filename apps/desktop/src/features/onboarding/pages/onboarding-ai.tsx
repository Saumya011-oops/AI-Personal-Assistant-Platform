import { useState } from 'react';
import { Link } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { ArrowLeft, ArrowRight, Cloud, Cpu, Sparkles, Loader2, Check, ShieldAlert, Key } from 'lucide-react';
import { useOnboarding, type AiSetup } from '../stores/onboarding-context';
import { invokeCommand } from '@/lib/api/invoke-command';

interface Option {
  key: AiSetup;
  title: string;
  Icon: any;
  tag?: string;
  body: string;
  bullets: string[];
}

const options: Option[] = [
  {
    key: 'groq',
    title: 'Groq',
    Icon: Cloud,
    tag: 'Fastest',
    body: 'Cloud inference for sub-second answers. Best when you have a network connection.',
    bullets: ['~300ms first token', 'Top-tier reasoning', 'Requires internet'],
  },
  {
    key: 'ollama',
    title: 'Ollama',
    Icon: Cpu,
    tag: 'Most private',
    body: 'Runs entirely on your machine. Zero network calls. Perfect for sensitive data.',
    bullets: ['100% local', 'Works offline', 'Uses your GPU/CPU'],
  },
  {
    key: 'hybrid',
    title: 'Hybrid',
    Icon: Sparkles,
    tag: 'Recommended',
    body: 'Groq when online, Ollama when offline or for confidential queries. Best of both.',
    bullets: ['Auto-switch', 'Privacy guardrails', 'Always available'],
  },
];

export function AiSetupStep() {
  const { ai, setAi } = useOnboarding();
  const [groqKey, setGroqKey] = useState('');
  const [ollamaUrl, setOllamaUrl] = useState('http://localhost:11434');

  const [saving, setSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState('');
  const [successMsg, setSuccessMsg] = useState('');

  const [groqKeySaved, setGroqKeySaved] = useState(false);
  const [ollamaUrlSaved, setOllamaUrlSaved] = useState(false);

  const handleSaveGroq = async () => {
    if (!groqKey.trim()) return;
    setSaving(true);
    setErrorMsg('');
    setSuccessMsg('');
    try {
      await invokeCommand('save_credential', { provider: 'groq', token: groqKey.trim() });
      setGroqKeySaved(true);
      setSuccessMsg('Groq API Key saved securely in SQLite.');
      setGroqKey('');
    } catch (err: any) {
      setErrorMsg(err?.message || 'Failed to save Groq API Key.');
    } finally {
      setSaving(false);
    }
  };

  const handleSaveOllama = async () => {
    if (!ollamaUrl.trim()) return;
    setSaving(true);
    setErrorMsg('');
    setSuccessMsg('');
    try {
      // Save Ollama URL in credentials table as well
      await invokeCommand('save_credential', { provider: 'ollama', token: ollamaUrl.trim() });
      setOllamaUrlSaved(true);
      setSuccessMsg('Ollama endpoint URL saved.');
    } catch (err: any) {
      setErrorMsg(err?.message || 'Failed to save Ollama URL.');
    } finally {
      setSaving(false);
    }
  };

  const isConfigurationComplete = (): boolean => {
    if (ai === 'groq') return groqKeySaved;
    if (ai === 'ollama') return ollamaUrlSaved || !!ollamaUrl; // Ollama has a default so it can bypass if desired
    if (ai === 'hybrid') return groqKeySaved && (ollamaUrlSaved || !!ollamaUrl);
    return false;
  };

  return (
    <div>
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4 }}
      >
        <h1 className="text-3xl font-bold text-gradient md:text-4xl">Choose your AI setup</h1>
        <p className="mt-3 text-sm text-outline md:text-base">
          Lumen can run in the cloud, on your machine, or both. Select your configuration.
        </p>
      </motion.div>

      <div className="mt-8 grid gap-3 md:grid-cols-3">
        {options.map((o, i) => {
          const active = ai === o.key;
          return (
            <button
              key={o.key}
              type="button"
              onClick={() => {
                setAi(o.key);
                setErrorMsg('');
                setSuccessMsg('');
              }}
              className={`relative flex flex-col rounded-2xl border p-5 text-left transition-all ${
                active
                  ? 'border-primary-glass bg-primary-glass/5 glow-sm'
                  : 'border-outline-variant/20 bg-surface-container-lowest/20 hover:bg-surface-container-high/30'
              }`}
            >
              {o.tag && (
                <span className="absolute right-4 top-4 rounded-full bg-primary-glass/10 px-2 py-0.5 text-[9px] font-bold uppercase tracking-wider text-primary-glass">
                  {o.tag}
                </span>
              )}
              <div
                className={`flex h-11 w-11 items-center justify-center rounded-xl transition-colors ${
                  active ? 'bg-primary-glass/15 text-primary-glass' : 'bg-surface-container-high text-outline'
                }`}
              >
                <o.Icon className="h-5 w-5" />
              </div>
              <h3 className="mt-5 text-base font-bold text-white">{o.title}</h3>
              <p className="mt-1 text-xs text-outline leading-relaxed">{o.body}</p>
              <ul className="mt-4 space-y-1.5 text-xs text-on-surface-variant font-mono">
                {o.bullets.map((b) => (
                  <li key={b} className="flex items-center gap-2">
                    <span className="h-1 w-1 rounded-full bg-primary-glass" />
                    {b}
                  </li>
                ))}
              </ul>
            </button>
          );
        })}
      </div>

      {/* Inputs for Groq / Ollama */}
      <AnimatePresence mode="wait">
        {(ai === 'groq' || ai === 'hybrid') && !groqKeySaved && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
            className="mt-6"
          >
            <div className="glass-strong rounded-2xl p-6 border border-primary-glass/20">
              <h3 className="text-xs font-bold text-white uppercase tracking-wider flex items-center gap-2 mb-3">
                <Key size={14} className="text-primary-glass" />
                Enter Groq API Key
              </h3>
              <p className="text-xs text-outline mb-3">
                To use Groq's super-fast cloud inference, please enter an API Key. This will be stored securely.
              </p>
              <div className="flex gap-2">
                <input
                  type="password"
                  placeholder="gsk_..."
                  value={groqKey}
                  onChange={e => setGroqKey(e.target.value)}
                  className="flex-1 rounded-xl border border-outline-variant/30 bg-surface-container-high/40 px-4 py-2.5 text-xs text-white focus:outline-none focus:border-primary-glass font-mono"
                />
                <button
                  type="button"
                  disabled={saving || !groqKey}
                  onClick={handleSaveGroq}
                  className="rounded-xl bg-primary-glass px-4 py-2.5 text-xs font-bold text-black hover:brightness-110 disabled:opacity-50 flex items-center gap-1.5 transition-all"
                >
                  {saving && <Loader2 size={13} className="animate-spin" />}
                  Save
                </button>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence mode="wait">
        {(ai === 'ollama' || ai === 'hybrid') && !ollamaUrlSaved && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
            className="mt-6"
          >
            <div className="glass-strong rounded-2xl p-6 border border-primary-glass/20">
              <h3 className="text-xs font-bold text-white uppercase tracking-wider flex items-center gap-2 mb-3">
                <Cpu size={14} className="text-primary-glass" />
                Configure Ollama Endpoint
              </h3>
              <p className="text-xs text-outline mb-3">
                Lumen uses Ollama for local text embeddings and fallback generation. Make sure Ollama is running.
              </p>
              <div className="flex gap-2">
                <input
                  type="text"
                  placeholder="http://localhost:11434"
                  value={ollamaUrl}
                  onChange={e => setOllamaUrl(e.target.value)}
                  className="flex-1 rounded-xl border border-outline-variant/30 bg-surface-container-high/40 px-4 py-2.5 text-xs text-white focus:outline-none focus:border-primary-glass font-mono"
                />
                <button
                  type="button"
                  disabled={saving || !ollamaUrl}
                  onClick={handleSaveOllama}
                  className="rounded-xl bg-primary-glass px-4 py-2.5 text-xs font-bold text-black hover:brightness-110 disabled:opacity-50 flex items-center gap-1.5 transition-all"
                >
                  {saving && <Loader2 size={13} className="animate-spin" />}
                  Save
                </button>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Success / Error Messages */}
      {errorMsg && (
        <div className="mt-4 flex items-center gap-2 text-red-400 text-xs font-mono glass p-4 rounded-xl">
          <ShieldAlert size={14} />
          {errorMsg}
        </div>
      )}
      {successMsg && (
        <div className="mt-4 flex items-center gap-2 text-green-400 text-xs font-mono glass p-4 rounded-xl">
          <Check size={14} />
          {successMsg}
        </div>
      )}

      {/* Navigation */}
      <div className="mt-10 flex items-center justify-between">
        <Link
          to="/onboarding/sources"
          className="inline-flex items-center gap-2 rounded-full glass px-4 py-2 text-xs font-bold text-white hover:bg-surface-container-high transition-all"
        >
          <ArrowLeft className="h-4 w-4" /> Back
        </Link>
        <Link
          to="/onboarding/indexing"
          className={`group inline-flex items-center gap-2 rounded-full bg-primary-glass px-6 py-3 text-sm font-bold text-black transition-all hover:glow-sm ${
            !isConfigurationComplete() ? 'opacity-50 pointer-events-none' : ''
          }`}
        >
          Continue
          <ArrowRight className="h-4 w-4 transition-transform group-hover:translate-x-0.5" />
        </Link>
      </div>
    </div>
  );
}
