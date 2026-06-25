import type { ReactNode } from "react";
import { post as howToChoose } from "@/content/blog/how-to-choose-a-kaspa-mining-pool";
import { post as toccataExplained } from "@/content/blog/kaspa-toccata-hard-fork-explained";
import { post as crescendoVsToccata } from "@/content/blog/kaspa-crescendo-vs-toccata";
import { post as howMiningWorks } from "@/content/blog/how-kaspa-mining-works";
import { post as isProfitable } from "@/content/blog/is-kaspa-mining-profitable-2026";
import { post as bestAsics } from "@/content/blog/most-profitable-kaspa-asic-miners-2026";
import { post as soloVsPool } from "@/content/blog/solo-vs-pool-mining-kaspa";
import { post as tokenomics } from "@/content/blog/kaspa-tokenomics-emission-explained";
import { post as rewardSchemes } from "@/content/blog/kaspa-pool-reward-schemes-explained";
import { post as payouts } from "@/content/blog/how-kaspa-pool-payouts-work";
import { post as whatIsNacho } from "@/content/blog/what-is-nacho-kaspa";
import { post as migrateHumpool } from "@/content/blog/migrate-from-humpool-to-kat-pool";

/**
 * Type-safe blog registry. Each post lives in its own module under
 * `src/content/blog/<slug>.tsx` and exports a `post: BlogPost`. This file is
 * the single aggregation point — the index page, dynamic `[slug]` route,
 * sitemap and OG images all read from `BLOG_POSTS`. Adding a post is: create
 * the module, then import it here. Order here is the default index order;
 * the index page sorts by `datePublished` regardless.
 */
export interface BlogPost {
  slug: string;
  title: string;
  description: string;
  datePublished: string;
  dateModified: string;
  readingMinutes: number;
  Body: () => ReactNode;
}

export const BLOG_POSTS: BlogPost[] = [
  toccataExplained,
  crescendoVsToccata,
  howMiningWorks,
  isProfitable,
  bestAsics,
  rewardSchemes,
  payouts,
  soloVsPool,
  tokenomics,
  whatIsNacho,
  migrateHumpool,
  howToChoose,
];

export function getPost(slug: string): BlogPost | undefined {
  return BLOG_POSTS.find((post) => post.slug === slug);
}
