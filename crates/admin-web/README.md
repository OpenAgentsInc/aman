# admin-web

## Responsibility

Admin web panel for Aman. Provides a lightweight dashboard and a broadcast tool for sending messages to
subscribers of selected topics.

## Features

- Dashboard with user counts, topic subscriber counts, and language stats
- Broadcast UI with preview (recipient list + counts)
- JSON APIs for stats and broadcast actions

## Configuration

Environment variables:

- `ADMIN_ADDR` (default: `127.0.0.1:8788`)
- `SQLITE_PATH` (default: `sqlite:aman.db?mode=rwc`)
- `SIGNAL_DAEMON_URL` (default: `http://127.0.0.1:8080`)
- `AMAN_NUMBER` (required)

## Run

```bash
export ADMIN_ADDR="127.0.0.1:8788"
export SQLITE_PATH="./data/aman.db"
export SIGNAL_DAEMON_URL="http://127.0.0.1:8080"
export AMAN_NUMBER="+15551234567"

cargo run -p admin-web
```

Open http://127.0.0.1:8788 in a browser.

## Routes

- `GET /` dashboard
- `GET /broadcast` broadcast form
- `GET /health` health check
- `GET /api/stats` stats JSON
- `GET /api/topics` topics JSON
- `POST /api/broadcast/preview` preview recipients
- `POST /api/broadcast` send broadcast

## Security notes

- No authentication is enforced in this crate. Bind to localhost or place it behind an authenticated
  reverse proxy if used outside a dev environment.
- Broadcasts send message text directly to Signal recipients; avoid logging message bodies.
