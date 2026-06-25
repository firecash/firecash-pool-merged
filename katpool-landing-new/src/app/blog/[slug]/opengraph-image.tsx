import { ogImage, ogSize, ogContentType } from "@/lib/og";
import { getPost } from "@/lib/blog";

export const size = ogSize;
export const contentType = ogContentType;
export const alt = "Kat Pool blog post";

export default async function Image({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const post = getPost(slug);
  const subtitle = post ? post.description.slice(0, 100) : "Kaspa mining guides & insights";
  return ogImage(post?.title ?? "Kat Pool blog", subtitle);
}
