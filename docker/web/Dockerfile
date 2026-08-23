# syntax=docker/dockerfile:1
FROM node:20.12.0 AS builder

ENV NODE_ENV=production

WORKDIR /app

ARG VERSION=main

RUN npm install -g pnpm@10.9.0
RUN git clone --depth 1 --branch ${VERSION} https://github.com/African-Research-Society/AppFlowy-Web.git .
RUN pnpm install
RUN env -u APPFLOWY_BASE_URL -u APPFLOWY_GOTRUE_BASE_URL -u APPFLOWY_WS_BASE_URL pnpm run build

FROM nginx:alpine
RUN apk add --no-cache tini

COPY --from=builder /app/dist /usr/share/nginx/html/
COPY --from=builder /app/docker/entrypoint.sh /docker-entrypoint.sh
COPY nginx.conf /etc/nginx/nginx.conf

RUN chmod +x /docker-entrypoint.sh

ENTRYPOINT ["/sbin/tini", "--", "/docker-entrypoint.sh"]
