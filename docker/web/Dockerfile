# syntax=docker/dockerfile:1
FROM node:20.12.0 AS builder

WORKDIR /app

ARG VERSION=main

RUN npm install -g pnpm@10.9.0
RUN git clone --depth 1 --branch ${VERSION} https://github.com/African-Research-Society/AppFlowy-Web.git .
RUN pnpm install
RUN pnpm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html/
COPY nginx.conf /etc/nginx/nginx.conf
