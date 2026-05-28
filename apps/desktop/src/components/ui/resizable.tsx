import {
  GripVertical,
} from 'lucide-react';
import type { ComponentProps } from 'react';
import { Group, Panel, Separator } from 'react-resizable-panels';

import { cn } from '@/lib/utils/cn';

type ResizablePanelGroupProps = Omit<ComponentProps<typeof Group>, 'orientation'> & {
  direction?: 'horizontal' | 'vertical';
};

export const ResizablePanelGroup = ({
  className,
  direction = 'horizontal',
  ...props
}: ResizablePanelGroupProps) => (
  <Group
    className={cn('flex h-full w-full data-[panel-group-direction=vertical]:flex-col', className)}
    orientation={direction}
    {...props}
  />
);

export const ResizablePanel = Panel;

export function ResizableHandle({
  className,
  withHandle,
  ...props
}: ComponentProps<typeof Separator> & { withHandle?: boolean }) {
  return (
    <Separator
      className={cn(
        'relative flex w-px items-center justify-center bg-border/70 after:absolute after:inset-y-0 after:left-1/2 after:w-3 after:-translate-x-1/2 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring/30 data-[panel-group-direction=vertical]:h-px data-[panel-group-direction=vertical]:w-full data-[panel-group-direction=vertical]:after:left-0 data-[panel-group-direction=vertical]:after:top-1/2 data-[panel-group-direction=vertical]:after:h-3 data-[panel-group-direction=vertical]:after:w-full data-[panel-group-direction=vertical]:after:-translate-y-1/2 data-[panel-group-direction=vertical]:after:translate-x-0',
        className,
      )}
      {...props}
    >
      {withHandle ? (
        <div className="z-10 flex h-8 w-4 items-center justify-center rounded-full border border-border bg-card text-muted-foreground data-[panel-group-direction=vertical]:h-4 data-[panel-group-direction=vertical]:w-8">
          <GripVertical className="h-3.5 w-3.5" />
        </div>
      ) : null}
    </Separator>
  );
}
