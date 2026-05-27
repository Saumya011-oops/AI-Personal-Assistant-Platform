import { createRoutesFromElements, Route, RouterProvider, createBrowserRouter } from 'react-router-dom';

import { AppShell } from '@/app/layout/app-shell';
import { DashboardPage } from '@/features/dashboard/pages/dashboard-page';
import { DocumentsPage } from '@/features/documents/pages/documents-page';
import { IntegrationsPage } from '@/features/integrations/pages/integrations-page';
import { SettingsPage } from '@/features/settings/pages/settings-page';

const router = createBrowserRouter(
  createRoutesFromElements(
    <Route element={<AppShell />} path="/">
      <Route element={<DashboardPage />} index />
      <Route element={<DocumentsPage />} path="documents" />
      <Route element={<IntegrationsPage />} path="integrations" />
      <Route element={<SettingsPage />} path="settings" />
    </Route>,
  ),
);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
