import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ArrowRight } from "lucide-react";
import { PageShell } from "@/components/site/page-shell";
import { JsonLd } from "@/components/json-ld";
import { breadcrumbLd, MAINTAINER } from "@/lib/structured-data";
import { BLOG_POSTS, getPost } from "@/lib/blog";
import { APP_URL } from "@/lib/mining";

const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "https://katpool.com";

export function generateStaticParams() {
  return BLOG_POSTS.map((post) => ({ slug: post.slug }));
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ slug: string }>;
}): Promise<Metadata> {
  const { slug } = await params;
  const post = getPost(slug);
  if (!post) return {};
  const url = `/blog/${post.slug}`;
  return {
    title: `${post.title} | Kat Pool`,
    description: post.description,
    alternates: { canonical: url },
    openGraph: {
      title: post.title,
      description: post.description,
      url,
      type: "article",
      publishedTime: post.datePublished,
      modifiedTime: post.dateModified,
    },
    twitter: { card: "summary_large_image", title: post.title, description: post.description },
  };
}

function articleLd(post: NonNullable<ReturnType<typeof getPost>>) {
  return {
    "@context": "https://schema.org",
    "@type": "BlogPosting",
    headline: post.title,
    description: post.description,
    inLanguage: "en",
    mainEntityOfPage: `${SITE_URL}/blog/${post.slug}`,
    datePublished: post.datePublished,
    dateModified: post.dateModified,
    author: { "@type": "Organization", name: "Kat Pool", url: SITE_URL },
    publisher: { "@id": `${SITE_URL}/#organization` },
  };
}

export default async function BlogPostPage({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const post = getPost(slug);
  if (!post) notFound();

  const { Body } = post;

  return (
    <PageShell>
      <JsonLd
        data={[
          articleLd(post),
          breadcrumbLd([
            { name: "Blog", path: "/blog" },
            { name: post.title, path: `/blog/${post.slug}` },
          ]),
        ]}
      />

      <nav aria-label="Breadcrumb" className="mb-6 text-xs text-muted-foreground">
        <Link href="/" className="transition hover:text-foreground">
          Home
        </Link>
        <span className="px-1.5">/</span>
        <Link href="/blog" className="transition hover:text-foreground">
          Blog
        </Link>
        <span className="px-1.5">/</span>
        <span className="text-foreground">{post.title}</span>
      </nav>

      <article className="space-y-8">
        <header className="space-y-4">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">{post.title}</h1>
          <p className="text-xs text-muted-foreground">
            By {MAINTAINER} · Updated{" "}
            <time dateTime={post.dateModified}>
              {new Date(post.dateModified).toLocaleDateString("en-US", {
                year: "numeric",
                month: "long",
                day: "numeric",
              })}
            </time>{" "}
            · {post.readingMinutes} min read
          </p>
        </header>

        <div className="space-y-6 leading-relaxed">
          <Body />
        </div>

        <section className="flex flex-col gap-3 rounded-2xl border border-primary/20 bg-primary/[0.06] p-6 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="text-lg font-semibold tracking-tight">Mine Kaspa on the open-source pool</h2>
            <p className="text-sm text-muted-foreground">Read the guide or open the live dashboard.</p>
          </div>
          <div className="flex gap-3">
            <Link
              href="/kaspa-mining-pool"
              className="inline-flex items-center gap-1.5 rounded-full border border-border px-4 py-2 text-sm text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
            >
              Mining guide
            </Link>
            <a
              href={APP_URL}
              className="inline-flex items-center gap-1.5 rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:brightness-110"
            >
              Open dashboard
              <ArrowRight className="size-3.5" />
            </a>
          </div>
        </section>
      </article>
    </PageShell>
  );
}
