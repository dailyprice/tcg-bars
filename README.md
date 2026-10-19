# tcgbars.com

A map of bars where it is good to sit down and play trading card games.
Anyone can add a bar, rate it on lighting, table space and offers for players,
and drag its pin onto the right spot.

## Layout

- `server/`: Rust API (axum + SQLite). It also serves the built frontend.
- `web/`: the map (MapLibre GL + Geoman, built with Vite).

## Running it locally

You need Rust and Node 20+.

```sh
# build the frontend once
cd web && npm install && npm run build && cd ..

# start the server from the repo root; it creates tcgbars.db and seeds the starter bars
cargo run --manifest-path server/Cargo.toml
```

Open http://127.0.0.1:8080.

For frontend work, run `npm run dev` in `web/` as well and use the Vite URL; it
forwards `/api` to the Rust server.

Settings (environment variables): `DATABASE_URL` (default `sqlite://tcgbars.db`),
`STATIC_DIR` (default `web/dist`), `PORT` (listen on `0.0.0.0:$PORT`) or
`BIND_ADDR` (default `127.0.0.1:8080`).

## API

| Method | Path | What it does |
| --- | --- | --- |
| GET | `/api/bars` | All bars as a GeoJSON FeatureCollection with average ratings |
| GET | `/api/bars/{id}` | One bar plus its 20 most recent ratings |
| POST | `/api/bars` | Add a bar: `{ name, lat, lng, address?, city?, country?, player_offers?, website? }` |
| PATCH | `/api/bars/{id}/location` | Move a pin: `{ lat, lng }` (marks the location as verified) |
| POST | `/api/bars/{id}/ratings` | Rate a bar: `{ lighting, table_space, offers, comment? }`, each 1 to 5 |
| GET | `/api/events` | Server-sent events (`bar_created`, `bar_moved`, `bar_rated`) for live map updates |
| GET | `/api/health` | Returns `ok` when the server and database are up |

## Tests

```sh
cargo test --manifest-path server/Cargo.toml
```

## Seed data

`server/seed/bars.json` holds the starter bars in Copenhagen. Pins without
`"verified": true` show as unverified until someone moves them onto the venue.

## Deploying on Railway

The repo includes a `Dockerfile` and `railway.toml`, so Railway builds and runs
it as is.

1. In Railway, create a project from this GitHub repo.
2. Add a volume to the service and mount it at `/data`. The SQLite database
   lives there; without a volume, every deploy starts from an empty database.
3. Under Networking, generate a domain (or add your own, e.g. tcgbars.com).

Railway sets `PORT` and checks `/api/health` before switching traffic to a new
deploy. Keep the service at one replica, since SQLite is a single file on one
volume.

To build the same image locally: `docker build -t tcgbars . && docker run -p 8080:8080 -v tcgbars-data:/data tcgbars`.
