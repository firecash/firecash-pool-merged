import type { Metadata } from "next";
import Link from "next/link";
import { PageShell } from "@/components/site/page-shell";
import { JsonLd } from "@/components/json-ld";
import { breadcrumbLd } from "@/lib/structured-data";
import { BLOG_POSTS } from "@/lib/blog";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";
const TITLE = "Kat Pool Blog — Kaspa Mining Guides & Insights";
const DESCRIPTION =
  "Guides, comparisons and insights on mining Kaspa (KAS): choosing a pool, profitability, hardware and getting the most from an open-source mining pool.";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  alternates: { canonical: "/blog" },
  openGraph: { title: TITLE, description: DESCRIPTION, url: "/blog", type: "website" },
  twitter: { card: "summary_large_image", title: TITLE, description: DESCRIPTION },
};

function blogLd() {
  return {
    "@context": "https://schema.org",
    "@type": "Blog",
    name: "Kat Pool Blog",
    url: `${SITE_URL}/blog`,
    inLanguage: "en",
    publisher: { "@id": `${SITE_URL}/#organization` },
    blogPost: BLOG_POSTS.map((post) => ({
      "@type": "BlogPosting",
      headline: post.title,
      description: post.description,
      url: `${SITE_URL}/blog/${post.slug}`,
      datePublished: post.datePublished,
      dateModified: post.dateModified,
    })),
  };
}

export default function BlogIndex() {
  const posts = [...BLOG_POSTS].sort((a, b) => b.datePublished.localeCompare(a.datePublished));

  return (
    <PageShell>
      <JsonLd data={[blogLd(), breadcrumbLd([{ name: "Blog", path: "/blog" }])]} />

      <nav aria-label="Breadcrumb" className="mb-6 text-xs text-muted-foreground">
        <Link href="/" className="transition hover:text-foreground">
          Home
        </Link>
        <span className="px-1.5">/</span>
        <span className="text-foreground">Blog</span>
      </nav>

      <header className="space-y-4">
        <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
          Kat Pool <span className="text-grad">blog</span>
        </h1>
        <p className="text-lg leading-relaxed text-muted-foreground">
          Guides, comparisons and insights on mining Kaspa (KAS) — from choosing a pool to estimating
          profitability on open-source infrastructure.
        </p>
      </header>

      <ul className="mt-10 space-y-4">
        {posts.map((post) => (
          <li key={post.slug}>
            <Link
              href={`/blog/${post.slug}`}
              className="glass-panel block rounded-2xl p-5 transition hover:border-primary/30"
            >
              <h2 className="text-lg font-semibold tracking-tight text-foreground">{post.title}</h2>
              <p className="mt-1.5 text-sm leading-relaxed text-muted-foreground">{post.description}</p>
              <p className="mt-3 text-xs text-muted-foreground">
                <time dateTime={post.datePublished}>
                  {new Date(post.datePublished).toLocaleDateString("en-US", {
                    year: "numeric",
                    month: "long",
                    day: "numeric",
                  })}
                </time>{" "}
                · {post.readingMinutes} min read
              </p>
            </Link>
          </li>
        ))}
      </ul>
    </PageShell>
  );
}
