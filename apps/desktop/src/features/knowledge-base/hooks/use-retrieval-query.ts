import { useState, useCallback } from 'react';
import { invokeCommand } from '@/lib/api/invoke-command';
import type {
  RetrievalRequest,
  RetrievalResponse,
  AllStrategiesResult,
} from '@assistant/shared';

// ─────────────────────────────────────────────────────────────────────────────
// useRetrievalQuery — debounced retrieval hook for the Knowledge Base page
// ─────────────────────────────────────────────────────────────────────────────

interface UseRetrievalQueryReturn {
  data: RetrievalResponse | null;
  isLoading: boolean;
  error: string | null;
  search: (request: RetrievalRequest) => void;
  clear: () => void;
}

export function useRetrievalQuery(): UseRetrievalQueryReturn {
  const [data, setData] = useState<RetrievalResponse | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const search = useCallback((request: RetrievalRequest) => {
    if (!request.query.trim()) {
      setData(null);
      setError(null);
      return;
    }

    setIsLoading(true);
    setError(null);

    invokeCommand('retrieve_documents', request)
      .then((result) => {
        setData(result);
        setIsLoading(false);
      })
      .catch((err) => {
        setError(String(err?.message ?? err));
        setIsLoading(false);
      });
  }, []);

  const clear = useCallback(() => {
    setData(null);
    setError(null);
    setIsLoading(false);
  }, []);

  return { data, isLoading, error, search, clear };
}

// ─────────────────────────────────────────────────────────────────────────────
// useAllStrategiesQuery — runs all 6 strategies for comparison mode
// ─────────────────────────────────────────────────────────────────────────────

interface UseAllStrategiesQueryReturn {
  data: AllStrategiesResult | null;
  isLoading: boolean;
  error: string | null;
  compare: (query: string, limit?: number) => void;
  clear: () => void;
}

export function useAllStrategiesQuery(): UseAllStrategiesQueryReturn {
  const [data, setData] = useState<AllStrategiesResult | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const compare = useCallback((query: string, limit = 5) => {
    if (!query.trim()) return;
    setIsLoading(true);
    setError(null);

    invokeCommand('test_retrieval_strategies', { query, limit })
      .then((result) => {
        setData(result);
        setIsLoading(false);
      })
      .catch((err) => {
        setError(String(err?.message ?? err));
        setIsLoading(false);
      });
  }, []);

  const clear = useCallback(() => {
    setData(null);
    setError(null);
    setIsLoading(false);
  }, []);

  return { data, isLoading, error, compare, clear };
}
