import { Command as CommandPrimitive } from 'cmdk';
import { Search } from 'lucide-react';
import type { ComponentProps, HTMLAttributes } from 'react';

import { cn } from '@/lib/utils/cn';

export function Command({
  className,
  ...props
}: ComponentProps<typeof CommandPrimitive>) {
  return (
    <CommandPrimitive
      className={cn(
        'flex h-full w-full flex-col overflow-hidden rounded-2xl bg-popover text-popover-foreground',
        className,
      )}
      {...props}
    />
  );
}

export function CommandInput({
  className,
  ...props
}: ComponentProps<typeof CommandPrimitive.Input>) {
  return (
    <div className="flex items-center border-b border-border px-4" cmdk-input-wrapper="">
      <Search className="mr-2 h-4 w-4 shrink-0 text-muted-foreground" />
      <CommandPrimitive.Input
        className={cn(
          'flex h-12 w-full rounded-md bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50',
          className,
        )}
        {...props}
      />
    </div>
  );
}

export const CommandList = CommandPrimitive.List;

export function CommandEmpty(props: ComponentProps<typeof CommandPrimitive.Empty>) {
  return <CommandPrimitive.Empty className="py-10 text-center text-sm text-muted-foreground" {...props} />;
}

export function CommandGroup({
  className,
  ...props
}: ComponentProps<typeof CommandPrimitive.Group>) {
  return (
    <CommandPrimitive.Group
      className={cn(
        'overflow-hidden p-2 text-foreground [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.2em] [&_[cmdk-group-heading]]:text-muted-foreground',
        className,
      )}
      {...props}
    />
  );
}

export function CommandItem({
  className,
  ...props
}: ComponentProps<typeof CommandPrimitive.Item>) {
  return (
    <CommandPrimitive.Item
      className={cn(
        'relative flex cursor-default select-none items-center gap-2 rounded-xl px-3 py-2.5 text-sm outline-none data-[selected=true]:bg-accent data-[selected=true]:text-accent-foreground data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-50',
        className,
      )}
      {...props}
    />
  );
}

export function CommandShortcut({
  className,
  ...props
}: HTMLAttributes<HTMLSpanElement>) {
  return <span className={cn('ml-auto text-xs tracking-[0.18em] text-muted-foreground', className)} {...props} />;
}
