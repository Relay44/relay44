import { WagmiAdapter } from '@reown/appkit-adapter-wagmi';
import { base as appKitBase, baseSepolia as appKitBaseSepolia } from '@reown/appkit/networks';
import { http, createConfig } from 'wagmi';
import { coinbaseWallet, injected, metaMask } from 'wagmi/connectors';
import { base, baseSepolia } from 'wagmi/chains';
import { BASE_CHAIN_ID, BASE_RPC_ENDPOINT } from '@/lib/constants';

const isTestnet = BASE_CHAIN_ID === baseSepolia.id;

const walletConnectors = [
  metaMask(),
  coinbaseWallet({ appName: 'Relay44', preference: { options: 'all' } }),
  injected({ shimDisconnect: false, target: 'phantom' }),
  injected({ shimDisconnect: false, target: 'rabby' }),
  injected({ shimDisconnect: true }),
];

const activeAppKitNetwork = isTestnet ? appKitBaseSepolia : appKitBase;
const appKitNetworks: [typeof activeAppKitNetwork] = [activeAppKitNetwork];

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL || 'https://relay44.com';

export const reownProjectId = process.env.NEXT_PUBLIC_REOWN_PROJECT_ID;

export const appKitMetadata = {
  name: 'Relay44',
  description: 'Onchain prediction markets with AI agents.',
  url: siteUrl,
  icons: [`${siteUrl}/favicon.ico`],
};

export const wagmiAdapter = reownProjectId
  ? new WagmiAdapter({
      connectors: walletConnectors,
      multiInjectedProviderDiscovery: false,
      projectId: reownProjectId,
      networks: appKitNetworks,
      ssr: true,
      transports: {
        [activeAppKitNetwork.id]: http(BASE_RPC_ENDPOINT),
      },
    })
  : undefined;

export const appKitConfig = wagmiAdapter
  ? {
      adapters: [wagmiAdapter],
      projectId: reownProjectId!,
      metadata: appKitMetadata,
      networks: appKitNetworks,
      themeMode: 'dark' as const,
      features: {
        analytics: true,
        email: false,
        socials: [],
      },
    }
  : null;

export const config =
  wagmiAdapter?.wagmiConfig ??
  createConfig({
    chains: [base, baseSepolia],
    connectors: walletConnectors,
    multiInjectedProviderDiscovery: false,
    ssr: true,
    transports: {
      [base.id]: http(BASE_CHAIN_ID === base.id ? BASE_RPC_ENDPOINT : undefined),
      [baseSepolia.id]: http(BASE_CHAIN_ID === baseSepolia.id ? BASE_RPC_ENDPOINT : undefined),
    },
  });
