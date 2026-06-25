/**
 * Renders a schema.org JSON-LD <script>. The payload is serialized and the `<`
 * character is escaped to `\u003c` to prevent any breakout from the script tag
 * (the sanitization pattern recommended by the Next.js JSON-LD guide).
 */
export function JsonLd({ data }: { data: object | object[] }) {
  return (
    <script
      type="application/ld+json"
      dangerouslySetInnerHTML={{
        __html: JSON.stringify(data).replace(/</g, "\\u003c"),
      }}
    />
  );
}
