#!/bin/bash
# Create the PostgreSQL database and user for EconWar.
# Run this once before starting the server.

set -e

DB_USER="econwar"
DB_PASS="econwar"
DB_NAME="econwar"

echo "Creating PostgreSQL user and database..."

# Create user (ignore error if already exists).
psql -U postgres -c "CREATE USER ${DB_USER} WITH PASSWORD '${DB_PASS}';" 2>/dev/null || true

# Create database.
psql -U postgres -c "CREATE DATABASE ${DB_NAME} OWNER ${DB_USER};" 2>/dev/null || true

# Grant privileges.
psql -U postgres -c "GRANT ALL PRIVILEGES ON DATABASE ${DB_NAME} TO ${DB_USER};"

echo "Database setup complete!"
echo "Connection string: postgres://${DB_USER}:${DB_PASS}@localhost:5432/${DB_NAME}"
