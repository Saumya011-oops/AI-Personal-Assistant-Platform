import * as AvatarPrimitive from '@radix-ui/react-avatar';
import type { ComponentProps } from 'react';

import { cn } from '@/lib/utils/cn';

export function Avatar({
  className,
  ...props
}: ComponentProps<typeof AvatarPrimitive.Root>) {
  return (
    <AvatarPrimitive.Root
      className={cn('relative flex h-9 w-9 shrink-0 overflow-hidden rounded-full', className)}
      {...props}
    />
  );
}

export function AvatarImage({
  className,
  ...props
}: ComponentProps<typeof AvatarPrimitive.Image>) {
  return <AvatarPrimitive.Image className={cn('aspect-square h-full w-full', className)} {...props} />;
}

export function AvatarFallback({
  className,
  ...props
}: ComponentProps<typeof AvatarPrimitive.Fallback>) {
  return (
    <AvatarPrimitive.Fallback
      className={cn('flex h-full w-full items-center justify-center rounded-full bg-secondary text-xs font-semibold text-secondary-foreground', className)}
      {...props}
    />
  );
}
