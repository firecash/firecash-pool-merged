export const runtime = "edge";

export async function GET() {
  return new Response("katpool landing ok", { status: 200 });
}
