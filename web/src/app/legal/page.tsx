import Link from 'next/link';
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/Card';
import { StructuredData } from '@/components/seo/StructuredData';
import {
  buildBreadcrumbStructuredData,
  buildCollectionPageStructuredData,
  buildPageMetadata,
} from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Legal',
  description: 'Terms, privacy, and platform risk disclosures for relay44.',
  path: '/legal',
  keywords: ['legal', 'terms', 'privacy policy', 'risk disclaimer'],
});

const legalPages = [
  {
    title: 'Terms of Service',
    description: 'Rules and conditions for using the relay44 platform',
    href: '/legal/terms',
  },
  {
    title: 'Privacy Policy',
    description: 'How we collect, use, and protect your data',
    href: '/legal/privacy',
  },
  {
    title: 'Risk Disclaimer',
    description: 'Important information about the risks of prediction markets',
    href: '/legal/disclaimer',
  },
];

export default function LegalPage() {
  return (
    <>
      <StructuredData
        data={[
          buildBreadcrumbStructuredData([
            { name: 'Home', url: '/' },
            { name: 'Legal', url: '/legal' },
          ]),
          buildCollectionPageStructuredData({
            path: '/legal',
            name: 'Legal documents',
            description: 'Terms, privacy policy, and risk disclosures for relay44.',
            items: legalPages.map((page) => ({ name: page.title, url: page.href })),
          }),
        ]}
      />
      <div className="container mx-auto px-4 pb-8 max-w-4xl">
        <h1 className="text-3xl font-bold text-text-primary mb-8">Legal</h1>

        <div className="grid gap-4">
          {legalPages.map((page) => (
            <Link key={page.href} href={page.href}>
              <Card hover>
                <CardHeader>
                  <CardTitle>{page.title}</CardTitle>
                  <CardDescription>{page.description}</CardDescription>
                </CardHeader>
              </Card>
            </Link>
          ))}
        </div>
      </div>
    </>
  );
}
