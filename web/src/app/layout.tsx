import type { Metadata, Viewport } from 'next';
import { Space_Grotesk, JetBrains_Mono, Inter } from 'next/font/google';
import './globals.css';
import { Providers } from '@/components/Providers';
import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildOrganizationStructuredData,
  buildRobots,
  buildWebApplicationStructuredData,
  buildWebsiteStructuredData,
  DEFAULT_DESCRIPTION,
  DEFAULT_KEYWORDS,
  SITE_HANDLE,
  SITE_IMAGE_ALT,
  SITE_IMAGE_PATH,
  SITE_NAME,
  SITE_URL,
} from '@/lib/seo';

const displayFont = Space_Grotesk({
  subsets: ['latin'],
  weight: ['500', '700'],
  variable: '--font-display-family',
});

const monoFont = JetBrains_Mono({
  subsets: ['latin'],
  weight: ['400', '700', '800'],
  variable: '--font-mono-family',
});

const bodyFont = Inter({
  subsets: ['latin'],
  weight: ['400', '600'],
  variable: '--font-body-family',
});

const googleVerification = process.env.NEXT_PUBLIC_GOOGLE_SITE_VERIFICATION?.trim();
const bingVerification = process.env.NEXT_PUBLIC_BING_SITE_VERIFICATION?.trim();
const yandexVerification = process.env.NEXT_PUBLIC_YANDEX_SITE_VERIFICATION?.trim();
const facebookVerification = process.env.NEXT_PUBLIC_FACEBOOK_DOMAIN_VERIFICATION?.trim();

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  applicationName: SITE_NAME,
  title: {
    default: 'Relay44 | prediction markets and agent execution',
    template: '%s | Relay44',
  },
  description: DEFAULT_DESCRIPTION,
  keywords: DEFAULT_KEYWORDS,
  category: 'finance',
  classification: 'Prediction markets',
  referrer: 'origin-when-cross-origin',
  creator: SITE_NAME,
  publisher: SITE_NAME,
  alternates: {
    canonical: SITE_URL,
  },
  formatDetection: {
    email: false,
    address: false,
    telephone: false,
  },
  verification: {
    google: googleVerification,
    yandex: yandexVerification,
    other: {
      ...(bingVerification ? { 'msvalidate.01': bingVerification } : {}),
      ...(facebookVerification ? { 'facebook-domain-verification': facebookVerification } : {}),
    },
  },
  manifest: '/manifest.json',
  icons: {
    icon: [
      { url: '/favicon.ico?v=20260310' },
      { url: '/favicon-48x48.png?v=20260310', sizes: '48x48', type: 'image/png' },
      { url: '/favicon-32x32.png?v=20260310', sizes: '32x32', type: 'image/png' },
      { url: '/favicon-16x16.png?v=20260310', sizes: '16x16', type: 'image/png' },
      { url: '/favicon.png?v=20260310', sizes: '512x512', type: 'image/png' },
    ],
    apple: [
      { url: '/apple-touch-icon.png?v=20260310', sizes: '180x180' },
      { url: '/apple-touch-icon-167x167.png?v=20260310', sizes: '167x167' },
      { url: '/apple-touch-icon-152x152.png?v=20260310', sizes: '152x152' },
    ],
  },
  openGraph: {
    title: 'Relay44 | prediction markets and agent execution',
    description: DEFAULT_DESCRIPTION,
    url: SITE_URL,
    siteName: SITE_NAME,
    locale: 'en_US',
    type: 'website',
    images: [
      {
        url: SITE_IMAGE_PATH,
        width: 1200,
        height: 630,
        alt: SITE_IMAGE_ALT,
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'Relay44 | prediction markets and agent execution',
    description: DEFAULT_DESCRIPTION,
    creator: SITE_HANDLE,
    site: SITE_HANDLE,
    images: [SITE_IMAGE_PATH],
  },
  robots: buildRobots(),
  appleWebApp: {
    capable: true,
    statusBarStyle: 'black-translucent',
    title: SITE_NAME,
  },
};

export const viewport: Viewport = {
  width: 'device-width',
  initialScale: 1,
  maximumScale: 1,
  userScalable: false,
  themeColor: '#030303',
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark" data-scroll-behavior="smooth">
      <head>
        <link rel="mask-icon" href="/relay44-logo-w.svg" color="#ffffff" />
        <meta name="msapplication-config" content="/browserconfig.xml" />
        <meta name="base:app_id" content="69b6fceed6271e8cedf2ada0" />
        <meta name="apple-mobile-web-app-capable" content="yes" />
        <meta
          name="fc:miniapp"
          content={JSON.stringify({
            version: '1',
            imageUrl: `${SITE_URL}/og-miniapp-relay44.svg`,
            button: {
              title: 'Open app',
              action: {
                type: 'launch_miniapp',
                name: 'Relay44',
                url: `${SITE_URL}/miniapp`,
                splashImageUrl: `${SITE_URL}/favicon.png`,
                splashBackgroundColor: '#030303',
              },
            },
          })}
        />
        <StructuredData
          data={[
            buildOrganizationStructuredData(),
            buildWebsiteStructuredData(),
            buildWebApplicationStructuredData(),
          ]}
        />
      </head>
      <body
        className={`${displayFont.variable} ${monoFont.variable} ${bodyFont.variable} antialiased`}
        style={{
          '--font-display': `${displayFont.style.fontFamily}, "Space Grotesk", sans-serif`,
          '--font-mono': `${monoFont.style.fontFamily}, "JetBrains Mono", ui-monospace, monospace`,
          '--font-sans': `${bodyFont.style.fontFamily}, "Inter", system-ui, sans-serif`,
        } as React.CSSProperties}
      >
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
