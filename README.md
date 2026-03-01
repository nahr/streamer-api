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

To reset the database (e.g. if `initialized` is wrong, or after schema changes like pool matches now using `camera_id`): delete `api/data/` and restart the API.

## Auth0 Login

Login uses Auth0. Configure in Auth0 dashboard:

1. Create an **Application** (Single Page Application) – note the Client ID.
2. Create an **API** – note the API Identifier (this is your audience).
3. In Application settings, add **Allowed Callback URLs**: `http://localhost:5173` (and your production URL).
4. Add **Allowed Logout URLs**: `http://localhost:5173` (and production).

Set in `.env` (same vars for UI and API):

- `AUTH0_DOMAIN` – your Auth0 domain (e.g. `your-tenant.us.auth0.com`)
- `AUTH0_CLIENT_ID` – SPA Application Client ID
- `AUTH0_AUDIENCE` – your API identifier

The first user to log in becomes an admin.

### Auth0 403 troubleshooting

1. **Check Auth0 Logs** – Dashboard → Monitoring → Logs. Reproduce the 403, then find the failed event. The log shows the exact reason (e.g. `fco` = origin not in Allowed Web Origins).

2. **URL consistency** – Don’t use the API’s “Test Application”; create a new **Single Page Application** in Applications → Create Application.

3. **API User Access** – In APIs → [your API] → Application Access, set **User Access** to **Allow** (not “Allow via client-grant”) so any app can get tokens for user login.

4. **Callback URLs** – Add `http://127.0.0.1:5173` and `http://localhost:5173` to Allowed Callback URLs, Allowed Logout URLs, and Allowed Web Origins.

5. **Use ID token** – Add `AUTH0_SKIP_AUDIENCE=true` to `.env` to skip the API audience.

6. **Wrong client ID** – If Auth0 receives a different client ID than in `.env`: shell env vars override `.env`; check for `.env.local` or `.env.development`; restart the dev server. In dev mode, the console logs `[Auth0] Client ID loaded: xxxxxxxx...` so you can verify.

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
2. In `.env`: `AUTH0_REDIRECT_URI=http://table-tv.local:5173`
3. In Auth0 Dashboard → Applications → [Your App] → Settings:
   - Add `http://table-tv.local:5173` to **Allowed Callback URLs**
   - Add `http://table-tv.local:5173` to **Allowed Logout URLs**
   - Add `http://table-tv.local:5173` to **Allowed Web Origins**
4. Open the app at **<http://table-tv.local:5173>** (not localhost)

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

Set `AUTH0_CONNECTION=facebook` in `.env` to skip the Auth0 method selection page.

**USB webcam:** If you use an external USB webcam instead of the built-in camera, set `CAMERA_INDEX=1` in `.env` (or `0` if the USB cam is the only/first device).

## RTMP streaming (Go Live)

RTMP export (YouTube, Facebook, etc.) uses **ffmpeg** to read the MJPEG stream and push to RTMP. The API requires ffmpeg to be installed and in `PATH`.

- **macOS:** `brew install ffmpeg`
- **Ubuntu/Debian:** `sudo apt install ffmpeg`
