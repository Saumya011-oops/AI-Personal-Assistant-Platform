import * as DialogPrimitive from '@radix-ui/react-dialog';
import type { PropsWithChildren } from 'react';

import { cn } from '@/lib/utils/cn';

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;

export function DialogContent({
  children,
  className,
}: PropsWithChildren<{ className?: string }>) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="fixed inset-0 bg-black/60 backdrop-blur-sm" />
      <DialogPrimitive.Content
        className={cn(
          'fixed left-1/2 top-1/2 w-[95vw] -translate-x-1/2 -translate-y-1/2 rounded-3xl border border-border bg-popover text-popover-foreground shadow-panel',
          className,
        )}
      >
        {children}
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
}
