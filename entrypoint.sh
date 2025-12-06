#!/bin/bash
set -e

# Default to GQL shell if no command provided
if [ $# -eq 0 ]; then
    # Check if database path is set and exists
    if [ -n "$GRAPHLITE_DB_PATH" ] && [ -d "$GRAPHLITE_DB_PATH" ]; then
        # Start GQL shell with database
        if [ -n "$GRAPHLITE_USER" ] && [ -n "$GRAPHLITE_PASSWORD" ]; then
            exec graphlite gql --path "$GRAPHLITE_DB_PATH" -u "$GRAPHLITE_USER" -p "$GRAPHLITE_PASSWORD"
        else
            echo "=========================================="
            echo "GraphLite Interactive GQL Shell"
            echo "=========================================="
            echo "Database path: $GRAPHLITE_DB_PATH"
            echo ""
            echo "Please provide credentials:"
            exec graphlite gql --path "$GRAPHLITE_DB_PATH"
        fi
    else
        # No database configured, show help
        echo "=========================================="
        echo "GraphLite - Graph Database"
        echo "=========================================="
        echo ""
        echo "No database configured. Please either:"
        echo "  1. Initialize a new database:"
        echo "     docker run -it -v \$(pwd)/mydb:/data graphlite:latest \\"
        echo "       graphlite install --path /data/mydb --admin-user admin --admin-password secret"
        echo ""
        echo "  2. Mount existing database and set environment variables:"
        echo "     docker run -it -v \$(pwd)/mydb:/data \\"
        echo "       -e GRAPHLITE_DB_PATH=/data/mydb \\"
        echo "       -e GRAPHLITE_USER=admin \\"
        echo "       -e GRAPHLITE_PASSWORD=secret \\"
        echo "       graphlite:latest"
        echo ""
        exec graphlite --help
    fi
else
    # Execute provided command
    exec "$@"
fi
