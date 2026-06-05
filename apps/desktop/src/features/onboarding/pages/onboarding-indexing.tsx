import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion } from 'framer-motion';
import { Check, Loader2, AlertCircle } from 'lucide-react';
import { useOnboarding } from '../stores/onboarding-context';
import { invokeCommand } from '@/lib/api/invoke-command';

const stages = [
  { key: 'fetch', label: 'Connecting & Syncing', body: 'Retrieving pages and documents from your active sources' },
  { key: 'chunk', label: 'Document Splitting', body: 'Normalizing and splitting text into semantic chunks' },
  { key: 'embed', label: 'Local Embedding Pipeline', body: 'Generating vector embeddings using Ollama' },
  { key: 'index', label: 'Vector Indexing', body: 'Writing nodes to Qdrant vector database' },
  { key: 'ready', label: 'Ready', body: 'Local knowledge graphs fully online' },
];

export function IndexingStep() {
  const navigate = useNavigate();
  const { sources } = useOnboarding();
  const [currentStage, setCurrentStage] = useState(0);
  const [progress, setProgress] = useState(0);
  const [docsCount, setDocsCount] = useState(0);
  const [errorMessage, setErrorMessage] = useState('');

  useEffect(() => {
    let active = true;

    const runSyncs = async () => {
      try {
        setCurrentStage(0); // Fetching
        setProgress(15);

        const promises = [];
        if (sources.includes('notion')) {
          promises.push(invokeCommand('sync_notion_documents', {}));
        }
        if (sources.includes('obsidian')) {
          promises.push(invokeCommand('scan_obsidian_vault', {}));
        }

        // Execute sync commands in parallel (if any)
        if (promises.length > 0) {
          const results = await Promise.all(promises);
          if (!active) return;
          
          // sum discovered docs
          const count = results.reduce((acc, r: any) => acc + (r?.documents_discovered || r?.documentsDiscovered || 0), 0);
          setDocsCount(count);
        } else {
          // If no syncable sources, simulate a quick load
          setDocsCount(12);
        }

        // Move to chunking stage
        setCurrentStage(1);
        setProgress(40);
        await new Promise((resolve) => setTimeout(resolve, 1500));
        if (!active) return;

        // Move to embedding stage
        setCurrentStage(2);
        setProgress(70);
        await new Promise((resolve) => setTimeout(resolve, 1500));
        if (!active) return;

        // Move to indexing stage
        setCurrentStage(3);
        setProgress(90);
        await new Promise((resolve) => setTimeout(resolve, 1000));
        if (!active) return;

        // Ready stage
        setCurrentStage(4);
        setProgress(100);
        await new Promise((resolve) => setTimeout(resolve, 800));
        if (!active) return;

        // Redirect to ready step
        navigate('/onboarding/ready');
      } catch (err: any) {
        if (active) {
          setErrorMessage(err?.message || 'Synchronization failed. Please check your token settings.');
        }
      }
    };

    runSyncs();

    return () => {
      active = false;
    };
  }, [sources, navigate]);

  return (
    <div>
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4 }}
      >
        <h1 className="text-3xl font-bold text-gradient md:text-4xl">
          Building your knowledge layer
        </h1>
        <p className="mt-3 text-sm text-outline md:text-base">
          This usually takes under a minute. Your data remains strictly local on your device.
        </p>
      </motion.div>

      <div className="mt-10 rounded-2xl glass-strong p-6 border border-primary-glass/10">
        <div className="flex items-end justify-between">
          <div>
            <div className="text-[10px] font-mono uppercase tracking-widest text-outline">Progress</div>
            <div className="mt-1 text-3xl font-bold text-gradient-cyan">
              {Math.round(progress)}%
            </div>
          </div>
          <div className="text-right">
            <div className="text-[10px] font-mono uppercase tracking-widest text-outline">
              Documents Found
            </div>
            <div className="mt-1 text-3xl font-bold text-white font-mono">
              {docsCount}
            </div>
          </div>
        </div>

        <div className="mt-5 h-2 overflow-hidden rounded-full bg-surface-container-highest">
          <motion.div
            animate={{ width: `${progress}%` }}
            transition={{ ease: 'linear', duration: 0.2 }}
            className="h-full rounded-full bg-gradient-to-r from-primary-glass/70 to-primary-glass glow-sm"
          />
        </div>

        {errorMessage ? (
          <div className="mt-6 p-4 rounded-xl border border-red-500/20 bg-red-500/5 text-red-400 text-xs font-mono flex items-start gap-2.5">
            <AlertCircle size={16} className="shrink-0 mt-0.5" />
            <div>
              <p className="font-bold mb-1">Sync Error</p>
              <p>{errorMessage}</p>
            </div>
          </div>
        ) : (
          <ul className="mt-7 space-y-2">
            {stages.map((s, i) => {
              const done = i < currentStage || progress >= 100;
              const active = i === currentStage && progress < 100;
              return (
                <li
                  key={s.key}
                  className={`flex items-center gap-3 rounded-xl px-3 py-2.5 transition-all ${
                    active ? 'bg-primary-glass/5' : ''
                  }`}
                >
                  <span
                    className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-xs font-bold ${
                      done
                        ? 'bg-primary-glass text-black'
                        : active
                        ? 'bg-primary-glass/10 text-primary-glass border border-primary-glass/20'
                        : 'bg-surface-container-high text-outline'
                    }`}
                  >
                    {done ? (
                      <Check className="h-4 w-4 stroke-[3]" />
                    ) : active ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      i + 1
                    )}
                  </span>
                  <div className="flex-1">
                    <div
                      className={`text-sm font-bold ${
                        done || active ? 'text-white' : 'text-outline'
                      }`}
                    >
                      {s.label}
                    </div>
                    <div className="text-xs text-outline">{s.body}</div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <div className="mt-6 text-center text-xs text-outline leading-relaxed">
        Tip: you can close this window at any time — indexing continues in the background.
      </div>
    </div>
  );
}
