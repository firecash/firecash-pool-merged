# syntax=docker/dockerfile:1

# ---- deps: install production-resolved node_modules ----------------------
FROM node:20-slim AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

# ---- builder: compile the Next standalone bundle -------------------------
FROM node:20-slim AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
ENV NEXT_TELEMETRY_DISABLED=1
# NEXT_PUBLIC_* values are inlined into the client bundle at build time, so
# they must be present during `npm run build`. Railway exposes service
# variables as build args only when declared with ARG (per its Dockerfile
# docs); promote them to ENV so Next picks them up. Server-only vars
# (KATPOOL_API_BASE_URL, KASPA_API_BASE_URL, COINGECKO_*) are read at runtime
# and intentionally NOT baked here.
ARG NEXT_PUBLIC_EXPLORER_BASE_URL
ARG NEXT_PUBLIC_POOL_NAME
ENV NEXT_PUBLIC_EXPLORER_BASE_URL=$NEXT_PUBLIC_EXPLORER_BASE_URL
ENV NEXT_PUBLIC_POOL_NAME=$NEXT_PUBLIC_POOL_NAME
RUN npm run build

# ---- runner: minimal runtime image --------------------------------------
FROM node:20-slim AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV PORT=3000
# Docker auto-sets HOSTNAME to the container ID; Next's standalone server uses
# `process.env.HOSTNAME || '0.0.0.0'`, so it would otherwise bind ONLY that
# single container-IP interface and be unreachable by Railway's healthcheck and
# private network. Bind the IPv6 wildcard, which Node opens dual-stack
# (IPv4 + IPv6), satisfying Railway's IPv4 healthcheck and IPv6 private
# networking alike (Railway's documented recommendation).
ENV HOSTNAME="::"

RUN addgroup --system --gid 1001 nodejs \
  && adduser --system --uid 1001 nextjs

COPY --from=builder /app/public ./public
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static

USER nextjs
EXPOSE 3000
CMD ["node", "server.js"]
