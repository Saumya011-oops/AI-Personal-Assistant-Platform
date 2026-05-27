import { z } from 'zod';

export const sidebarSectionSchema = z.object({
  id: z.string(),
  label: z.string(),
  href: z.string(),
  icon: z.string(),
});

export type SidebarSection = z.infer<typeof sidebarSectionSchema>;
