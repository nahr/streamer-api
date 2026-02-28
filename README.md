# Table TV

A simple app with an API and UI, served together.

## Quick Start (Docker)

1. Build and run:

   ```bash
   docker compose up --build
   ```

2. Open in your browser:
   - **<http://localhost>** or **<http://127.0.0.1>**
   - For **<http://table-tv.local>**, add to `/etc/hosts`: `127.0.0.1 table-tv.local`

## Local Development

**Terminal 1 – API:**

```bash
cd api && cargo run
```

**Terminal 2 – UI:**

```bash
cd ui && npm run dev
```

The UI proxies `/api` to the API. Open <http://localhost:5173>.
