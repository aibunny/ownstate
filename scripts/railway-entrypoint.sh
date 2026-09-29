#!/bin/sh
set -eu

# Railway volumes are mounted after the image is built and may initially be
# owned by root. Prepare the fixed model-cache mount, then permanently drop
# privileges before starting any Ownstate process.
if [ "$(id -u)" -eq 0 ]; then
    cache_dir="${OWNSTATE_EMBEDDING_CACHE_DIR:-/app/.fastembed_cache}"
    case "$cache_dir" in
        /app/.fastembed_cache|/app/.fastembed_cache/*) ;;
        *)
            echo "OWNSTATE_EMBEDDING_CACHE_DIR must be /app/.fastembed_cache or a child path" >&2
            exit 64
            ;;
    esac
    mkdir -p "$cache_dir"
    chown -R ownstate:ownstate /app/.fastembed_cache
    exec gosu ownstate "$0" "$@"
fi

case "${OWNSTATE_PROCESS:-api}" in
    api)
        exec /usr/local/bin/ownstate-api
        ;;
    worker)
        exec /usr/local/bin/ownstate-worker
        ;;
    mcp)
        exec /usr/local/bin/ownstate-mcp
        ;;
    *)
        echo "OWNSTATE_PROCESS must be api, worker, or mcp" >&2
        exit 64
        ;;
esac
