# ops/db — database provisioning

Out-of-band Postgres provisioning that is **not** part of the application's
`sqlx` schema migrations (`crates/katpool-db/migrations/`). Role and privilege
management lives here, not in migrations, because (a) it needs a role with
`CREATEROLE`/ownership that the app's migration user may not have, and (b) login
credentials must never be committed.

## `api-readonly-role.sql` — least-privilege API role (ADR-0021)

Creates the `katpool_readonly` group role (SELECT-only on `public`, current and
future tables) so the embedded read-only HTTP API can connect with no write
access, isolated from the accountant/payout writers.

```sh
# As the table owner / migration role, once per environment:
psql "$KATPOOL_DATABASE_URL" -f ops/db/api-readonly-role.sql

# Then (out-of-band) create the login user and set the API's URL:
#   CREATE ROLE katpool_api LOGIN PASSWORD '<secret>';
#   GRANT katpool_readonly TO katpool_api;
#   export KATPOOL_API_DATABASE_URL=postgres://katpool_api:<secret>@host:port/db
```

When `KATPOOL_API_DATABASE_URL` is unset the API shares the writers' pool
(unchanged behaviour) — the read-only split is opt-in per deployment.
