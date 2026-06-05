import { motion } from 'framer-motion';
import { ArrowRight, PlayCircle, Sparkles } from 'lucide-react';
import { Link } from 'react-router-dom';
import type { SVGProps } from 'react';

import {
  NotionLogo,
  DriveLogo,
  GmailLogo,
  CalendarLogo,
  ObsidianLogo,
} from './logos-helper';

type Source = {
  Icon: (props: SVGProps<SVGSVGElement>) => React.ReactElement;
  label: string;
  color: string;
};

const sources: Source[] = [
  { Icon: NotionLogo, label: 'Notion', color: '#8ed5ff' },
  { Icon: DriveLogo, label: 'Google Drive', color: '#eab308' },
  { Icon: GmailLogo, label: 'Gmail', color: '#f87171' },
  { Icon: CalendarLogo, label: 'Calendar', color: '#60a5fa' },
  { Icon: ObsidianLogo, label: 'Obsidian', color: '#c084fc' },
];

export function WelcomeStep() {
  return (
    <div className="py-6 select-none">
      <div className="grid items-center gap-8 lg:grid-cols-[1.1fr_1fr]">
        {/* Left column info */}
        <div className="text-left">
          <motion.div
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
            className="inline-flex items-center gap-2 rounded-full glass px-3.5 py-1 text-xs text-outline"
          >
            <span className="inline-flex h-1.5 w-1.5 animate-pulse-glow rounded-full bg-primary-glass shadow-[0_0_8px_rgba(142,213,255,0.6)]" />
            Local-first AI Workspace · v1.0
          </motion.div>

          <motion.h1
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6, delay: 0.05 }}
            className="mt-6 text-4xl font-extrabold leading-[1.1] text-gradient md:text-5xl"
          >
            Your entire digital knowledge base.
            <br />
            <span className="text-gradient-cyan">One intelligent assistant.</span>
          </motion.h1>

          <motion.p
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6, delay: 0.15 }}
            className="mt-6 max-w-lg text-sm leading-relaxed text-outline md:text-base"
          >
            Search and chat across Notion, Obsidian, Gmail, Google Drive, Calendars, and local files using advanced retrieval strategies. Private, secure, and offline-capable.
          </motion.p>

          <motion.div
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6, delay: 0.25 }}
            className="mt-8 flex flex-wrap items-center gap-4"
          >
            <Link
              to="/onboarding/sources"
              className="group inline-flex items-center gap-2 rounded-full bg-primary-glass px-6 py-3.5 text-sm font-bold text-black transition-all hover:glow shadow-lg"
            >
              Get started
              <ArrowRight className="h-4.5 w-4.5 transition-transform group-hover:translate-x-0.5" />
            </Link>
            <a
              href="#"
              onClick={(e) => e.preventDefault()}
              className="inline-flex items-center gap-2 rounded-full glass px-6 py-3.5 text-sm font-medium text-white transition-colors hover:bg-surface-container-high"
            >
              <PlayCircle className="h-4 w-4 text-primary-glass" />
              Watch overview
            </a>
          </motion.div>

          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.8, delay: 0.4 }}
            className="mt-12 flex flex-wrap items-center gap-x-6 gap-y-3 text-xs text-outline font-mono"
          >
            <span className="inline-flex items-center gap-1.5">
              <span className="h-1.5 w-1.5 rounded-full bg-primary-glass" /> Runs locally
            </span>
            <span className="inline-flex items-center gap-1.5">
              <span className="h-1.5 w-1.5 rounded-full bg-primary-glass" /> Zero cloud lock-in
            </span>
            <span className="inline-flex items-center gap-1.5">
              <span className="h-1.5 w-1.5 rounded-full bg-primary-glass" /> Secure SQLite storage
            </span>
          </motion.div>
        </div>

        {/* Right column: visual interactive flow */}
        <KnowledgeFlow />
      </div>
    </div>
  );
}

function KnowledgeFlow() {
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.7, delay: 0.2 }}
      className="relative mx-auto w-full max-w-[440px]"
    >
      <div className="relative aspect-square">
        {/* Background ambient glow */}
        <div className="absolute inset-10 rounded-full bg-primary-glass/5 blur-3xl" />

        {/* Source nodes (left column) */}
        <div className="absolute inset-y-0 left-0 flex w-1/3 flex-col justify-around py-4 z-10">
          {sources.map(({ Icon, label, color }, i) => (
            <motion.div
              key={label}
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.5, delay: 0.3 + i * 0.08 }}
              className="flex items-center gap-2.5"
            >
              <div
                className="flex h-10 w-10 items-center justify-center rounded-xl glass animate-float"
                style={{ color, animationDelay: `${i * 0.4}s` }}
              >
                <Icon className="h-5 w-5" />
              </div>
              <span className="text-xs text-outline font-medium">{label}</span>
            </motion.div>
          ))}
        </div>

        {/* SVG Flowing connections */}
        <svg
          className="absolute inset-0 h-full w-full pointer-events-none"
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
        >
          <defs>
            <linearGradient id="line-grad" x1="0" y1="0" x2="1" y2="0">
              <stop offset="0%" stopColor="#8ed5ff" stopOpacity="0" />
              <stop offset="50%" stopColor="#8ed5ff" stopOpacity="0.4" />
              <stop offset="100%" stopColor="#8ed5ff" stopOpacity="0.8" />
            </linearGradient>
          </defs>
          {[12, 31, 50, 69, 88].map((y, i) => (
            <motion.path
              key={i}
              d={`M 32 ${y} Q 50 ${y}, 54 50`}
              stroke="url(#line-grad)"
              strokeWidth="0.4"
              fill="none"
              initial={{ pathLength: 0, opacity: 0 }}
              animate={{ pathLength: 1, opacity: 1 }}
              transition={{ duration: 1.2, delay: 0.5 + i * 0.1 }}
            />
          ))}
          {/* Out lines to results */}
          <motion.path
            d="M 66 50 Q 78 50, 88 30"
            stroke="url(#line-grad)"
            strokeWidth="0.5"
            fill="none"
            initial={{ pathLength: 0 }}
            animate={{ pathLength: 1 }}
            transition={{ duration: 1, delay: 1.2 }}
          />
          <motion.path
            d="M 66 50 Q 78 50, 88 70"
            stroke="url(#line-grad)"
            strokeWidth="0.5"
            fill="none"
            initial={{ pathLength: 0 }}
            animate={{ pathLength: 1 }}
            transition={{ duration: 1, delay: 1.3 }}
          />
          {/* Flowing animated circles */}
          {[0, 1, 2, 3].map((i) => (
            <motion.circle
              key={i}
              r="0.5"
              fill="#8ed5ff"
              initial={{ offsetDistance: '0%' }}
              animate={{ offsetDistance: '100%' }}
              transition={{ duration: 3, delay: i * 0.7, repeat: Infinity, ease: 'linear' }}
              style={{
                offsetPath: `path("M 32 ${[12, 31, 50, 69][i % 4]} Q 50 ${[12, 31, 50, 69][i % 4]}, 54 50")`,
              }}
            />
          ))}
        </svg>

        {/* Center element: AI core */}
        <motion.div
          initial={{ scale: 0.8, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          transition={{ duration: 0.6, delay: 0.6 }}
          className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-10"
        >
          <div className="relative">
            <div className="absolute inset-0 -m-4 rounded-full bg-primary-glass/20 blur-xl animate-pulse-glow" />
            <div className="relative flex h-24 w-24 flex-col items-center justify-center rounded-2xl glass-strong glow">
              <Sparkles className="h-5 w-5 text-primary-glass" />
              <div className="mt-1.5 text-[10px] font-bold tracking-widest text-white">
                AI CORE
              </div>
              <div className="text-[9px] text-outline font-mono mt-0.5">RAG · Hybrid</div>
            </div>
          </div>
        </motion.div>

        {/* Right side outputs */}
        <div className="absolute inset-y-0 right-0 flex w-1/3 flex-col items-end justify-around py-12 z-10">
          <motion.div
            initial={{ opacity: 0, x: 16 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: 1.4 }}
            className="rounded-xl glass px-3 py-2.5 text-right text-[10px]"
          >
            <div className="text-white font-bold">Grounded answer</div>
            <div className="text-outline font-mono mt-0.5">with citations</div>
          </motion.div>
          <motion.div
            initial={{ opacity: 0, x: 16 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: 1.6 }}
            className="rounded-xl glass px-3 py-2.5 text-right text-[10px]"
          >
            <div className="text-white font-bold">Source trace</div>
            <div className="text-outline font-mono mt-0.5">secure local index</div>
          </motion.div>
        </div>
      </div>
    </motion.div>
  );
}
