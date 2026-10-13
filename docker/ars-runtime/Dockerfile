# Build context contains only the reviewed native ARM binary, never runtime secrets.
FROM debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
RUN apt-get update && apt-get install -y --no-install-recommends openssl ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY appflowy_cloud /usr/local/bin/appflowy_cloud
ARG SOURCE_REVISION
LABEL org.opencontainers.image.revision=$SOURCE_REVISION
ENV APP_ENVIRONMENT=production RUST_BACKTRACE=1 PORT=8000
EXPOSE 8000
CMD ["appflowy_cloud"]
