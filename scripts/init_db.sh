#!/usr/bin/env bash
set -x
set -eo pipefail

# Check if sqlx-cli is installed
if ! command -v sqlx &> /dev/null
then
    echo >&2 "Error: sqlx is not installed."
    echo >&2 "Use:"
    echo >&2 "    cargo install --version='~0.8' sqlx-cli --no-default-features --features rustls,postgres"
    echo >&2 "to install it."
    exit 1
fi

# Check if a custom user has been set, otherwise default to 'postgres'
DB_USER="${POSTGRES_USER:=postgres}"
# Check if a custom password has been set, otherwise default to 'password'
DB_PASSWORD="${POSTGRES_PASSWORD:=password}"
# Check if a custom database name has been set, otherwise default to 'newsletter'
DB_NAME="${POSTGRES_DB:=newsletter}"
# Check if a custom port has been set, otherwise default to '5432'
DB_PORT="${POSTGRES_PORT:=5432}"
# Check if a custom host has been set, otherwise default to 'localhost'
DB_HOST="${POSTGRES_HOST:=localhost}"

# Allow to skip Docker if a dockerized Postgres database is already running
if [[ -z "${SKIP_DOCKER}" ]]
then
    CONTAINER_NAME="postgres_$(date +%s)"
    docker run \
        --name "${CONTAINER_NAME}" \
        -e POSTGRES_USER=${DB_USER} \
        -e POSTGRES_PASSWORD=${DB_PASSWORD} \
        -e POSTGRES_DB=${DB_NAME} \
        -p "${DB_PORT}":5432 \
        -d postgres \
        postgres -N 1000
        # ^ Increased maximum number of connections for testing purposes

    # Wait for Postgres to be ready
    until docker exec "${CONTAINER_NAME}" pg_isready -U "${DB_USER}" -d "${DB_NAME}"; do
        echo >&2 "Postgres is still unavailable - sleeping"
        sleep 1
    done
fi

echo >&2 "Postgres is up and running on port ${DB_PORT}!"

DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}
export DATABASE_URL
sqlx database create
sqlx migrate run
    
echo >&2 "Postgres has been migrated, ready to go!"
