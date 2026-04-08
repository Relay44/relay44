import { useEffect, useState } from 'react';
import { isAdminWallet } from '@/lib/admin';

export function useAdminGate(address: string | undefined | null) {
  const [isAdmin, setIsAdmin] = useState(false);

  useEffect(() => {
    let mounted = true;
    if (!address) {
      setIsAdmin(false);
      return;
    }
    isAdminWallet(address).then((result) => {
      if (mounted) setIsAdmin(result);
    });
    return () => {
      mounted = false;
    };
  }, [address]);

  return isAdmin;
}
