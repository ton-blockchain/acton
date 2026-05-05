import { useCallback, useEffect, useState } from 'react';

export type Network = 'mainnet' | 'testnet';

interface Route {
  isTestnet: boolean;
}

function parseRoute(): Route {
  const params = new URLSearchParams(window.location.search);
  return { isTestnet: params.get('testnet') === 'true' };
}

function buildUrl(testnet: boolean) {
  const params = new URLSearchParams();
  if (testnet) params.set('testnet', 'true');
  const search = params.toString();
  return search ? `/?${search}` : '/';
}

function push(url: string) {
  if (window.location.pathname + window.location.search !== url) {
    history.pushState(null, '', url);
    window.dispatchEvent(new Event('routechange'));
  }
}

export function useRouter() {
  const [route, setRoute] = useState<Route>(parseRoute);

  useEffect(() => {
    const update = () => setRoute(parseRoute());
    window.addEventListener('popstate', update);
    window.addEventListener('routechange', update);
    return () => {
      window.removeEventListener('popstate', update);
      window.removeEventListener('routechange', update);
    };
  }, []);

  const setTestnet = useCallback((testnet: boolean) => {
    push(buildUrl(testnet));
  }, []);

  return {
    network: (route.isTestnet ? 'testnet' : 'mainnet') as Network,
    setTestnet,
  };
}
