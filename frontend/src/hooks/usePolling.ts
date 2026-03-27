import { useEffect, useState, useCallback, useRef } from 'react';

interface UsePollingOptions {
  interval: number;
  enabled?: boolean;
  pauseOnHidden?: boolean;
  pauseOnOffline?: boolean;
}

interface UsePollingResult {
  isPolling: boolean;
  isPaused: boolean;
  pauseReason: 'hidden' | 'offline' | 'manual' | null;
  pause: () => void;
  resume: () => void;
  forceRefresh: () => void;
}

export function usePolling(
  refetchFn: () => Promise<unknown>,
  options: UsePollingOptions
): UsePollingResult {
  const {
    interval,
    enabled = true,
    pauseOnHidden = true,
    pauseOnOffline = true,
  } = options;

  const [isPolling, setIsPolling] = useState(false);
  const [manualPause, setManualPause] = useState(false);
  const [isHidden, setIsHidden] = useState(
    typeof document !== 'undefined' && document.hidden
  );
  const [isOffline, setIsOffline] = useState(
    typeof navigator !== 'undefined' && !navigator.onLine
  );

  const refetchRef = useRef(refetchFn);
  const isRefetchingRef = useRef(false);

  useEffect(() => {
    refetchRef.current = refetchFn;
  }, [refetchFn]);

  useEffect(() => {
    if (typeof document === 'undefined') return;

    const handleVisibilityChange = () => {
      setIsHidden(document.hidden);
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, []);

  useEffect(() => {
    if (typeof window === 'undefined') return;

    const handleOnline = () => setIsOffline(false);
    const handleOffline = () => setIsOffline(true);

    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);
    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, []);

  const shouldPoll =
    enabled &&
    !manualPause &&
    !(pauseOnHidden && isHidden) &&
    !(pauseOnOffline && isOffline);

  const getPauseReason = useCallback((): 'hidden' | 'offline' | 'manual' | null => {
    if (manualPause) return 'manual';
    if (pauseOnHidden && isHidden) return 'hidden';
    if (pauseOnOffline && isOffline) return 'offline';
    return null;
  }, [manualPause, isHidden, isOffline, pauseOnHidden, pauseOnOffline]);

  const doRefetch = useCallback(async () => {
    if (isRefetchingRef.current) return;
    isRefetchingRef.current = true;
    try {
      await refetchRef.current();
    } finally {
      isRefetchingRef.current = false;
    }
  }, []);

  useEffect(() => {
    if (!shouldPoll || interval <= 0) {
      setIsPolling(false);
      return;
    }

    setIsPolling(true);

    const pollInterval = setInterval(() => {
      doRefetch();
    }, interval);

    return () => {
      clearInterval(pollInterval);
      setIsPolling(false);
    };
  }, [shouldPoll, interval, doRefetch]);

  const pause = useCallback(() => {
    setManualPause(true);
  }, []);

  const resume = useCallback(() => {
    setManualPause(false);
  }, []);

  const forceRefresh = useCallback(() => {
    doRefetch();
  }, [doRefetch]);

  return {
    isPolling,
    isPaused: !shouldPoll && enabled,
    pauseReason: enabled ? getPauseReason() : null,
    pause,
    resume,
    forceRefresh,
  };
}
