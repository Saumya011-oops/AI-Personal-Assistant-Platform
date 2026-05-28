import * as SeparatorPrimitive from '@radix-ui/react-separator';
import type { ComponentProps } from 'react';

import { cn } from '@/lib/utils/cn';

export function Separator({
  className,
  orientation = 'horizontal',
  decorative = true,
}: ComponentProps<typeof SeparatorPrimitive.Root>) {
  return (
    <SeparatorPrimitive.Root
      className={cn(
        'shrink-0 bg-border',
        orientation === 'horizontal' ? 'h-px w-full' : 'h-full w-px',
        className,
      )}
      decorative={decorative}
      orientation={orientation}
    />
  );
}
