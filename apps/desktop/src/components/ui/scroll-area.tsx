import * as ScrollAreaPrimitive from '@radix-ui/react-scroll-area';
import type { ComponentProps } from 'react';

import { cn } from '@/lib/utils/cn';

export function ScrollArea({
  className,
  children,
  ...props
}: ComponentProps<typeof ScrollAreaPrimitive.Root>) {
  return (
    <ScrollAreaPrimitive.Root className={cn('relative overflow-hidden', className)} {...props}>
      <ScrollAreaPrimitive.Viewport className="h-full w-full rounded-[inherit]">
        {children}
      </ScrollAreaPrimitive.Viewport>
      <ScrollBar />
      <ScrollAreaPrimitive.Corner />
    </ScrollAreaPrimitive.Root>
  );
}

function ScrollBar({
  className,
  orientation = 'vertical',
  ...props
}: ComponentProps<typeof ScrollAreaPrimitive.ScrollAreaScrollbar>) {
  return (
    <ScrollAreaPrimitive.ScrollAreaScrollbar
      className={cn(
        'flex touch-none select-none rounded-full p-0.5 transition-colors',
        orientation === 'vertical' ? 'h-full w-2.5 border-l border-l-transparent' : 'h-2.5 flex-col border-t border-t-transparent',
        className,
      )}
      orientation={orientation}
      {...props}
    >
      <ScrollAreaPrimitive.ScrollAreaThumb className="relative flex-1 rounded-full bg-border/80" />
    </ScrollAreaPrimitive.ScrollAreaScrollbar>
  );
}
