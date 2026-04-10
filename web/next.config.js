/** @type {import('next').NextConfig} */
const path = require('path');

const disablePwaByEnv = ['1', 'true', 'yes', 'on'].includes(
  String(process.env.NEXT_PUBLIC_DISABLE_PWA || '')
    .trim()
    .toLowerCase()
);
const disablePwa = process.env.NODE_ENV === 'development' || disablePwaByEnv;

const withPWA = require('next-pwa')({
  dest: 'public',
  register: true,
  skipWaiting: true,
  disable: disablePwa,
});

const nextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  distDir: process.env.NODE_ENV === 'development' ? '.next-dev' : '.next',
  outputFileTracingRoot: path.join(__dirname, '..'),
  eslint: {
    ignoreDuringBuilds: true,
  },

  images: {
    // Modern formats for better compression
    formats: ['image/avif', 'image/webp'],

    // Remote patterns for external images
    remotePatterns: [
      // GitHub avatars (user profile images)
      {
        protocol: 'https',
        hostname: 'avatars.githubusercontent.com',
        pathname: '/u/**',
      },
      // Unsplash (used in FeaturedBanner)
      {
        protocol: 'https',
        hostname: 'images.unsplash.com',
        pathname: '/**',
      },
      // Arweave gateway (NFT/market images)
      {
        protocol: 'https',
        hostname: 'arweave.net',
        pathname: '/**',
      },
      // IPFS gateways
      {
        protocol: 'https',
        hostname: 'ipfs.io',
        pathname: '/ipfs/**',
      },
      {
        protocol: 'https',
        hostname: 'cloudflare-ipfs.com',
        pathname: '/ipfs/**',
      },
      {
        protocol: 'https',
        hostname: 'gateway.pinata.cloud',
        pathname: '/ipfs/**',
      },
    ],

    // Device sizes for srcset generation
    deviceSizes: [375, 640, 750, 828, 1080, 1200, 1440, 1920, 2048],

    // Image sizes for responsive images
    imageSizes: [16, 32, 48, 64, 96, 128, 256, 384],

    // Minimum cache TTL (1 week)
    minimumCacheTTL: 604800,

    // Disable static image imports if needed for optimization
    // disableStaticImages: false,
  },

  webpack: (config) => {
    config.externals.push('pino-pretty', 'lokijs', 'encoding');
    config.resolve.alias = {
      ...(config.resolve.alias || {}),
      '@react-native-async-storage/async-storage': false,
      '@worldcoin/idkit': path.resolve(__dirname, 'node_modules/@worldcoin/idkit/build/index.js'),
    };
    config.resolve.fallback = {
      ...config.resolve.fallback,
      fs: false,
      net: false,
      tls: false,
      // Optional peer dependency of @wagmi/connectors' MetaMask connector.
      // The connector wraps the import() in a try/catch and throws a runtime
      // error only if a caller actually uses MetaMask without the SDK installed.
      // Mapping it to `false` silences webpack's missing-module warning without
      // breaking any consumer that does install it.
      '@metamask/connect-evm': false,
    };
    return config;
  },

  async rewrites() {
    return [
      {
        source: '/.well-known/farcaster.json',
        destination: '/api/well-known/farcaster',
      },
    ];
  },

  async headers() {
    const csp = [
      "default-src 'self'",
      "script-src 'self' 'unsafe-inline' 'unsafe-eval' blob: https://*.tradingview.com https://esm.sh",
      "style-src 'self' 'unsafe-inline'",
      "img-src 'self' data: blob: https:",
      "font-src 'self' data: https:",
      "connect-src 'self' https: wss:",
      "frame-src 'self' https://*.tradingview.com https://*.tradingview-widget.com",
      "frame-ancestors 'self' https://farcaster.xyz https://*.farcaster.xyz https://warpcast.com https://*.warpcast.com https://*.coinbase.com https://*.base.org https://base.dev https://www.base.dev",
      "base-uri 'self'",
      "form-action 'self'",
    ].join('; ');

    const sharedHeaders = [
      { key: 'X-Content-Type-Options', value: 'nosniff' },
      { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
      {
        key: 'Permissions-Policy',
        value: 'camera=(), microphone=(), geolocation=(), payment=()',
      },
      { key: 'Content-Security-Policy', value: csp },
      { key: 'Strict-Transport-Security', value: 'max-age=31536000; includeSubDomains; preload' },
    ];

    return [
      {
        // Miniapp routes: no X-Frame-Options so Base App / Warpcast can embed
        source: '/miniapp/:path*',
        headers: sharedHeaders,
      },
      {
        // All other routes: keep X-Frame-Options for security
        source: '/((?!miniapp).*)',
        headers: [
          { key: 'X-Frame-Options', value: 'SAMEORIGIN' },
          ...sharedHeaders,
        ],
      },
    ];
  },
};

module.exports = withPWA(nextConfig);
