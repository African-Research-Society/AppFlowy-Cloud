# nginx with the Supabase-auth config baked in.
#
# Why not just bind-mount nginx/nginx.supabase.conf? Coolify rewrites relative
# bind mounts in a compose file into managed "file storage" entries under
# /data/coolify/applications/<uuid>/ and creates the host path itself. For a
# path it has no stored content for it creates a *directory*, and mounting a
# directory onto /etc/nginx/nginx.conf fails the container at startup:
#
#   error mounting ".../nginx/nginx.supabase.conf" to rootfs at
#   "/etc/nginx/nginx.conf": not a directory
#
# Shipping the config in the image sidesteps that machinery entirely. The
# pre-existing ssl bind mounts are left alone — Coolify already has real files
# for those paths.
#
# Build context is the repository root (see docker-compose.override.yml).

FROM nginx

COPY nginx/nginx.supabase.conf /etc/nginx/nginx.conf
