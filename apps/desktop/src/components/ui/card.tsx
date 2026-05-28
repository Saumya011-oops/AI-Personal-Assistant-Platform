import type { PropsWithChildren } from 'react';

import { cn } from '@/lib/utils/cn';

export function Card({
  children,
  className,
}: PropsWithChildren<{ className?: string }>) {
  return (
    <div
      className={cn(
        'rounded-3xl border border-border/80 bg-card/88 p-5 shadow-subtle backdrop-blur',
        className,
      )}
    >
      {children}
    </div>
  );
}
