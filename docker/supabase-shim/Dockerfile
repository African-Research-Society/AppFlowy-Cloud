# Postgres with the shadow-auth initdb shim baked in.
#
# Same reason as docker/supabase-nginx/Dockerfile: a relative bind mount of
# 00-auth-shim.sql becomes a Coolify-managed path that is created as an empty
# directory, and /docker-entrypoint-initdb.d/00-auth-shim.sql silently becomes
# a directory inside the container — postgres initialises without ever running
# the shim, and AppFlowy's FK migration then fails on the missing auth schema.
#
# Baking it in makes the shim a property of the image, so it cannot go missing.
#
# Build context is the repository root (see docker-compose.override.yml).

FROM pgvector/pgvector:pg16

COPY docker/supabase-shim/00-auth-shim.sql /docker-entrypoint-initdb.d/00-auth-shim.sql
