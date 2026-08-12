# SoraLabs Email Rust

Experimental Rust implementation of the SoraLabs
email service running on Cloudflare Workers.

The current production implementation is TypeScript:
[SoraLabsOSS/email](https://github.com/SoraLabsOSS/email).

```
POST /api/contact          POST /api/newsletter
       ↓                          ↓
    Queue (email-rust-queue)  Resend Contacts API
       ↓
 Queue Consumer
       ↓
   Resend Email API
```

Rate limiting (Upstash) is enabled (fixed-window via Upstash Redis REST).

## Clone and run

### 1. Prerequisites

- Rust (`rustup`) — [rustup.rs](https://rustup.rs/)
- `wasm32-unknown-unknown` target
- Node.js (for Wrangler)

```sh
rustup target add wasm32-unknown-unknown
```

### 2. Clone

```sh
git clone https://github.com/SoraLabsOSS/email-rust.git
cd email-rust
```

### 3. Local secrets

```sh
cp .dev.vars.example .dev.vars
```

Fill in:

| Secret           | Description                       |
| ---------------- | --------------------------------- |
| `API_KEYS`       | Comma-separated keys for child apps |
| `RESEND_API_KEY` | From [Resend](https://resend.com) |
| `UPSTASH_REDIS_REST_URL` | From [Upstash Redis](https://console.upstash.com) |
| `UPSTASH_REDIS_REST_TOKEN` | From Upstash Redis |

Inbox / from-address / CORS / optional `RESEND_NEWSLETTER_SEGMENT_ID` live in `wrangler.toml` (`[vars]`).

### 4. Dev

From this repo directory (not `$HOME`):

```sh
npm install
npm run dev
```

Use the pinned Wrangler in `package.json` (`npm run dev` / `npm run deploy`), not a bare `npx wrangler`.

```sh
curl http://localhost:8787/health

curl -X POST http://localhost:8787/api/contact \
  -H "Authorization: Bearer <API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{"name":"Jane","email":"jane@example.com","message":"Hi","app":"landing"}'

curl -X POST http://localhost:8787/api/newsletter \
  -H "Authorization: Bearer <API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{"email":"jane@example.com"}'
```

Local queues are simulated. You do not need to create Cloudflare queues for `wrangler dev`.

## API

### `GET /` and `GET /health`

```json
{ "ok": true, "service": "email-rust", "time": "..." }
```

### `POST /api/contact`

**Headers**

```
Authorization: Bearer <API_KEY>
Content-Type: application/json
```

`X-API-Key` is also accepted.

**Body**

```json
{
  "name": "Jane Doe",
  "email": "jane@example.com",
  "message": "Hello from the landing page",
  "subject": "Website inquiry",
  "app": "landing"
}
```

| Field     | Required | Notes                         |
| --------- | -------- | ----------------------------- |
| `name`    | yes      | max 100                       |
| `email`   | yes      | Reply-To on the outbound mail |
| `message` | yes      | max 5000                      |
| `subject` | no       | default `New contact message` |
| `app`     | no       | tags the email; default `default` |

**Responses**

- `202` accepted and queued
- `400` validation / invalid JSON
- `401` missing or invalid API key
- `403` app not in allowlist
- `405` method not allowed
- `502` queue enqueue failed
- `503` `API_KEYS` secret not bound

### `POST /api/newsletter`

Adds the address to Resend Contacts (`unsubscribed: false`). Does not send mail.

Only `email` is required. Name fields may be omitted or empty.

**Headers** — same as `/api/contact`.

**Body**

```json
{ "email": "jane@example.com" }
```

| Field       | Required | Notes                                            |
| ----------- | -------- | ------------------------------------------------ |
| `email`     | yes      | subscriber address                               |
| `firstName` | no       | Resend `first_name`; omit or `""` is fine        |
| `lastName`  | no       | Resend `last_name`; omit or `""` is fine         |
| `name`      | no       | split into first/last if `firstName` is omitted  |
| `app`       | no       | child-app id for allowlist / logs; default `default` |

**Responses**

- `201` created (or already subscribed — `alreadyExists: true`)
- `400` validation / invalid JSON
- `401` missing or invalid API key
- `403` app not in allowlist
- `405` method not allowed
- `502` Resend create failed
- `503` `API_KEYS` secret not bound

## Deploy

Push to `main` runs [GitHub Actions](https://developers.cloudflare.com/workers/ci-cd/external-cicd/github-actions/)
(`.github/workflows/deploy.yml` → `wrangler deploy`). Do **not** connect this
repo in Cloudflare Workers Builds — that would duplicate CI and skip Rust cache.

- `https://email-rust.truonggiang-axyl.workers.dev`

Queues (`email-rust-queue`) and secrets (`API_KEYS`, `RESEND_API_KEY`) already
live on the Worker. Do not reuse the TypeScript queues (`email-queue`).
