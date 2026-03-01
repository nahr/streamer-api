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

**Terminal 1 – API** (auto-reloads on changes; requires [cargo-watch](https://crates.io/crates/cargo-watch): `cargo install cargo-watch`):

```bash
cd api && cargo watch -x run
```

**Terminal 2 – UI:**

```bash
cd ui && npm run dev
```

The UI proxies `/api` to the API. Open <http://localhost:5173>.

To reset the database (e.g. if `initialized` is wrong): delete `api/data/` and restart the API.

**USB webcam:** If you use an external USB webcam instead of the built-in camera, set `CAMERA_INDEX=1` in `.env` (or `0` if the USB cam is the only/first device).

## RTMP streaming (Go Live)

RTMP export (YouTube, Facebook, etc.) uses **ffmpeg** to read the MJPEG stream and push to RTMP. The API requires ffmpeg to be installed and in `PATH`.

- **macOS:** `brew install ffmpeg`
- **Ubuntu/Debian:** `sudo apt install ffmpeg`
