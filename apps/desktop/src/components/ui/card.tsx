import type { PropsWithChildren } from 'react';

import { cn } from '@/lib/utils/cn';

export function Card({
  children,
  className,
}: PropsWithChildren<{ className?: string }>) {
  return (
    <div
      className={cn(
        'rounded-3xl border border-border/60 bg-card/70 p-5 shadow-[0_8px_30px_rgba(0,0,0,0.18)] backdrop-blur',
        className,
      )}
    >
      {children}
    </div>
  );
}
