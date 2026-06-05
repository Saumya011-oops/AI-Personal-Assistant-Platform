import { createContext, useContext, useState, type ReactNode } from 'react';

export type SourceKey =
  | 'notion'
  | 'drive'
  | 'gmail'
  | 'gcal'
  | 'apple-cal'
  | 'obsidian'
  | 'local';

export type AiSetup = 'groq' | 'ollama' | 'hybrid';

interface OnboardingContextType {
  sources: SourceKey[];
  ai: AiSetup;
  toggleSource: (k: SourceKey) => void;
  addSource: (k: SourceKey) => void;
  removeSource: (k: SourceKey) => void;
  setAi: (a: AiSetup) => void;
}

const OnboardingCtx = createContext<OnboardingContextType | null>(null);

export function OnboardingProvider({ children }: { children: ReactNode }) {
  const [sources, setSources] = useState<SourceKey[]>([]);
  const [ai, setAi] = useState<AiSetup>('hybrid');

  const toggleSource = (k: SourceKey) =>
    setSources((s) => (s.includes(k) ? s.filter((x) => x !== k) : [...s, k]));

  const addSource = (k: SourceKey) =>
    setSources((s) => (s.includes(k) ? s : [...s, k]));

  const removeSource = (k: SourceKey) =>
    setSources((s) => s.filter((x) => x !== k));

  return (
    <OnboardingCtx.Provider value={{ sources, ai, toggleSource, addSource, removeSource, setAi }}>
      {children}
    </OnboardingCtx.Provider>
  );
}

export function useOnboarding() {
  const v = useContext(OnboardingCtx);
  if (!v) {
    throw new Error('useOnboarding must be used within an OnboardingProvider');
  }
  return v;
}
