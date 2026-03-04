# Table TV

 A simple app with an API and UI, served together. Runs on Linux with ffmpeg, MediaMTX, stunnel, and Avahi installed as needed.

## Quick Start

**Terminal 1 – API:**

```bash
cd api && cargo run
```

**Terminal 2 – UI:**

```bash
cd ui && npm run dev
```

The UI proxies `/api` to the API. Open <http://localhost:5173>.

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

To reset the database (e.g. if `initialized` is wrong, or after schema changes): delete `api/data/` and restart the API.

## Auth0 Login

Login uses Auth0. Configure in Auth0 dashboard:

1. Create an **Application** (Single Page Application) – note the Client ID.
2. Create an **API** – note the API Identifier (this is your audience).
3. In Application settings, add **Allowed Callback URLs**: `http://localhost:5173` (and your production URL).
4. Add **Allowed Logout URLs**: `http://localhost:5173` (and production).

Set in `table-tv.config` (same for UI and API). Copy `table-tv.config.example` to `table-tv.config` and add:

```toml
[auth0]
domain = "your-tenant.us.auth0.com"
client_id = "your-spa-client-id"
audience = "https://your-api-identifier"
```

The first user to log in becomes an admin.

### Auth0 403 troubleshooting

1. **Check Auth0 Logs** – Dashboard → Monitoring → Logs. Reproduce the 403, then find the failed event. The log shows the exact reason (e.g. `fco` = origin not in Allowed Web Origins).

2. **URL consistency** – Don’t use the API’s “Test Application”; create a new **Single Page Application** in Applications → Create Application.

3. **API User Access** – In APIs → [your API] → Application Access, set **User Access** to **Allow** (not “Allow via client-grant”) so any app can get tokens for user login.

4. **Callback URLs** – Add `http://127.0.0.1:5173` and `http://localhost:5173` to Allowed Callback URLs, Allowed Logout URLs, and Allowed Web Origins.

5. **Use ID token** – Add `skip_audience = true` to `[auth0]` in table-tv.config to skip the API audience.

6. **Wrong client ID** – If Auth0 receives a different client ID than in table-tv.config: ensure the correct table-tv.config is used (project root, `api/`, or `/etc/table-tv/`); restart the dev server. In dev mode, the console logs `[Auth0] Client ID loaded: xxxxxxxx...` so you can verify.

### "Facebook User" or "Google User" instead of real name

When the app shows a generic name instead of your real name, the JWT lacks profile claims. The backend tries Auth0's userinfo endpoint, but this can fail (rate limits 429, or when using `skip_audience` the access token may not be available). **The recommended fix is the Auth0 Action below** – it adds profile data to the ID token so userinfo is never needed.

### Auth0 claims (username, email, profile picture)

The app requests `scope: 'openid profile email'`, which includes standard OIDC claims: `name`, `nickname`, `email`, `picture`. For social logins (Facebook, Google), these may be empty if the identity provider doesn’t share them.

**To add or fix claims in the token:**

1. **Auth0 Actions** – Dashboard → Actions → Flows → Login. Add a new Action that runs on “Login / Post Login”:

   ```javascript
   exports.onExecutePostLogin = async (event, api) => {
     const user = event.user;
     const name = user.name || (user.identities?.[0]?.profile_data?.name);
     if (name) api.idToken.setCustomClaim('name', name);
     if (user.email) api.idToken.setCustomClaim('email', user.email);
     if (user.picture) api.idToken.setCustomClaim('picture', user.picture);
     if (user.nickname) api.idToken.setCustomClaim('nickname', user.nickname);
   };
   ```

2. **Log out and log back in** – The Action only runs on new logins; your current token won't have the claims until you sign in again.

3. **Social connection settings** – Dashboard → Authentication → Social. For each connection (Facebook, Google, etc.), ensure the requested attributes include name, email, and profile picture.

4. **Facebook** – In the Facebook connection, use only `public_profile` and `email`. Remove `user_link` and any other invalid scopes. If you see "Invalid Scopes: email, user_link":
   - **Auth0**: Dashboard → Authentication → Social → Facebook → edit the connection. Set permissions to `public_profile,email` only.
   - **Meta for Developers**: Your Facebook app → Use cases → Authentication and account creation → add the `email` permission if needed.

### Simplify login flow (skip "Authorize app" and "Reconnect" prompts)

If you see multiple prompts: "Continue with Facebook" → "Reconnect to table.tv" (Facebook) → "Authorize app" (Auth0):

#### Auth0 "Authorize app" – cannot skip on localhost

Auth0 **always shows consent for `localhost`** – this is a security restriction and cannot be overridden. To skip it during development:

1. Add to `/etc/hosts`: `127.0.0.1 table-tv.local`
2. In Auth0 Dashboard → Applications → [Your App] → Settings:
   - Add `http://table-tv.local:5173` to **Allowed Callback URLs**
   - Add `http://table-tv.local:5173` to **Allowed Logout URLs**
   - Add `http://table-tv.local:5173` to **Allowed Web Origins**
3. Open the app at **<http://table-tv.local:5173>** (not localhost; redirect URI is derived from the URL)

Also enable: Auth0 Dashboard → APIs → [your API] → Settings → Access Settings → **Allow Skipping User Consent**.

#### Facebook "Reconnect to table.tv"

1. **Valid OAuth Redirect URIs** – In [Meta for Developers](https://developers.facebook.com/) → Your App → Facebook Login → Settings, add:

   ```
   https://YOUR_TENANT.auth0.com/login/callback
   ```

   Replace `YOUR_TENANT` with your Auth0 domain (e.g. `dev-r1xdk6f2gw5bybyr`).

2. **App mode** – If the app is in **Development** mode, only test users can log in and Facebook may show different prompts. Switch to **Live** mode (App Review → Permissions and Features) if you need all users to log in.

3. **App domains** – Add your domain (e.g. `table-tv.local` or your production domain) to **App Domains** in the Facebook app's Basic Settings.

#### Go straight to Facebook

Set `connection = "facebook"` in `[auth0]` in table-tv.config to skip the Auth0 method selection page.

**RTSP cameras:** Add a camera with type RTSP and the stream URL (e.g. `rtsp://192.168.1.100:554/stream`). The stream, match overlay, and RTMP Go Live all work for RTSP. Requires ffmpeg.

### MediaMTX rolling recording

[MediaMTX](https://github.com/bluenviron/mediamtx) can run alongside Table TV to:

- **Proxy camera streams** – MJPEG preview and FFmpeg (Facebook Live) read from MediaMTX instead of the camera, so the camera has a single connection and isn’t overloaded.
- **Record rolling video** – Each camera is recorded in segments. Configure in **Server Settings → Rolling Video Storage**:
  - **Record path** – Where to store recordings (empty = `./recordings` relative to MediaMTX)
- **Segment duration** – Length per file (e.g. `1m`, `30m`, `1h`). First file appears after this duration.
- **Delete after** – Retention (e.g. `24h`, `7d`; empty = keep forever)

Cameras are synced to MediaMTX automatically when added or updated.

### Test RTSP stream (MediaMTX + FFmpeg)

To test RTSP without a real camera, use [MediaMTX](https://github.com/bluenviron/mediamtx) to receive the stream and FFmpeg to publish a test pattern.

1. Create mediamtx.yml:

```yml
paths:
  test:
    source: publisher
```

1. **Terminal 1 – MediaMTX** (from project root):

   ```bash
   mediamtx mediamtx.yml
   ```

2. **Terminal 2 – FFmpeg** (publish a test pattern):

   ```bash
   ffmpeg -re -f lavfi -i "testsrc=size=1280x720:rate=30" -c:v libx264 -pix_fmt yuv420p -preset ultrafast -f rtsp rtsp://localhost:8554/test
   ```

3. In the app, add an RTSP camera with URL `rtsp://localhost:8554/test`.

## RTMP streaming (Go Live)

RTMP export (YouTube, Facebook, etc.) uses **ffmpeg** to read the RTSP stream and push to RTMP. Works for RTSP cameras. The API requires ffmpeg to be installed and in `PATH`.

- **Ubuntu/Debian:** `sudo apt install ffmpeg`

### Facebook Live "Input/output error" – stunnel workaround

FFmpeg's native RTMPS support often fails when streaming to Facebook Live. Use **stunnel** as a relay:

1. **Install stunnel**
   - Ubuntu/Debian: `sudo apt install stunnel4`

2. **Create stunnel config** (e.g. `stunnel-fb.conf`):

   ```
   [fb-live]
   client = yes
   accept = 127.0.0.1:19350
   connect = live-api-s.facebook.com:443
   verifyChain = no
   ```

3. **Run stunnel** before going live:

   ```bash
   stunnel stunnel-fb.conf
   ```

4. **Set env and start the API:**

   ```bash
   USE_STUNNEL_FOR_RTMPS=1 cargo run
   ```

   The API will send the stream to `rtmp://127.0.0.1:19350`, and stunnel will forward it over TLS to Facebook.
