FROM node:22-bookworm-slim AS deps

WORKDIR /app/web

ENV NEXT_TELEMETRY_DISABLED=1

COPY web/package.json web/package-lock.json ./
RUN npm ci

FROM node:22-bookworm-slim AS builder

WORKDIR /app/web

ENV NEXT_TELEMETRY_DISABLED=1

COPY --from=deps /app/web/node_modules ./node_modules
COPY web ./

RUN npm run build

FROM node:22-bookworm-slim AS runner

WORKDIR /app/web

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV HOSTNAME=0.0.0.0
ENV PORT=10000

COPY --from=builder /app/web ./

EXPOSE 10000

CMD ["npm", "run", "start"]
