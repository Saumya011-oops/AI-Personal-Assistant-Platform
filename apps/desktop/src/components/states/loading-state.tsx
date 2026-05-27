export function LoadingState({ label }: { label: string }) {
  return (
    <div className="rounded-3xl border border-border/60 bg-card/70 p-8">
      <div className="h-3 w-40 animate-pulse rounded-full bg-white/10" />
      <div className="mt-4 h-24 animate-pulse rounded-2xl bg-white/5" />
      <p className="mt-4 text-sm text-slate-400">{label}</p>
    </div>
  );
}
