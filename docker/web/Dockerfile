# syntax=docker/dockerfile:1
FROM node:20.12.0 AS builder

ENV NODE_ENV=production

WORKDIR /app

ARG VERSION=main
ARG REVISION=9d872b6c8bd8db71edf96149ed41c383ca986524

RUN npm install -g pnpm@10.9.0
RUN git init . \
    && git remote add origin https://github.com/African-Research-Society/AppFlowy-Web.git \
    && git fetch --depth 1 origin "${REVISION:-$VERSION}" \
    && git checkout --detach FETCH_HEAD
RUN pnpm install
RUN env -u APPFLOWY_BASE_URL -u APPFLOWY_GOTRUE_BASE_URL -u APPFLOWY_WS_BASE_URL pnpm run build

FROM nginx:alpine
RUN apk add --no-cache tini

COPY --from=builder /app/dist /usr/share/nginx/html/
COPY --from=builder /app/docker/entrypoint.sh /docker-entrypoint.sh
COPY --from=builder /app/docker/nginx.conf /etc/nginx/nginx.conf

RUN chmod +x /docker-entrypoint.sh

ENTRYPOINT ["/sbin/tini", "--", "/docker-entrypoint.sh"]
