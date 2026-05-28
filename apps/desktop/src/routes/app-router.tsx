import { Suspense, lazy, type ReactNode } from 'react';
import { createRoutesFromElements, Route, RouterProvider, createBrowserRouter } from 'react-router-dom';

import { AppShell } from '@/app/layout/app-shell';
import { LoadingState } from '@/components/states/loading-state';

const HomePage = lazy(async () => ({
  default: (await import('@/features/home/pages/home-page')).HomePage,
}));

const AssistantPage = lazy(async () => ({
  default: (await import('@/features/assistant/pages/assistant-page')).AssistantPage,
}));

const DocumentsPage = lazy(async () => ({
  default: (await import('@/features/documents/pages/documents-page')).DocumentsPage,
}));

const IntegrationsPage = lazy(async () => ({
  default: (await import('@/features/integrations/pages/integrations-page')).IntegrationsPage,
}));

const SettingsPage = lazy(async () => ({
  default: (await import('@/features/settings/pages/settings-page')).SettingsPage,
}));

const router = createBrowserRouter(
  createRoutesFromElements(
    <Route element={<AppShell />} path="/">
      <Route
        element={<LazyPage><HomePage /></LazyPage>}
        handle={{ title: 'Home', subtitle: 'Workspace overview' }}
        index
      />
      <Route
        element={<LazyPage><AssistantPage /></LazyPage>}
        handle={{ title: 'Assistant', subtitle: 'Grounded conversation workspace' }}
        path="assistant"
      />
      <Route
        element={<LazyPage><DocumentsPage /></LazyPage>}
        handle={{ title: 'Knowledge Base', subtitle: 'Split-view document explorer' }}
        path="documents"
      />
      <Route
        element={<LazyPage><IntegrationsPage /></LazyPage>}
        handle={{ title: 'Integrations', subtitle: 'Sources and sync health' }}
        path="integrations"
      />
      <Route
        element={<LazyPage><SettingsPage /></LazyPage>}
        handle={{ title: 'Settings', subtitle: 'Workspace preferences and auth' }}
        path="settings"
      />
    </Route>,
  ),
);

export function AppRouter() {
  return <RouterProvider router={router} />;
}

function LazyPage({ children }: { children: ReactNode }) {
  return (
    <Suspense fallback={<LoadingState label="Loading workspace surface." />}>
      {children}
    </Suspense>
  );
}
