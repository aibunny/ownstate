#!/bin/sh
set -eu

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
