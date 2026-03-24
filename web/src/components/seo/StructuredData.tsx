interface StructuredDataProps {
  data?: Record<string, unknown> | Array<Record<string, unknown> | null | undefined> | null;
}

function serialize(value: Record<string, unknown>) {
  return JSON.stringify(value).replace(/</g, '\\u003c');
}

export function StructuredData({ data }: StructuredDataProps) {
  const entries = Array.isArray(data)
    ? data.filter((entry): entry is Record<string, unknown> => Boolean(entry))
    : data
      ? [data]
      : [];

  if (entries.length === 0) {
    return null;
  }

  return (
    <>
      {entries.map((entry, index) => (
        <script
          key={index}
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: serialize(entry) }}
        />
      ))}
    </>
  );
}
