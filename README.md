# SoraLabs Email Rust

Experimental Rust implementation of the SoraLabs
email service running on Cloudflare Workers.

The current production implementation is TypeScript:
[SoraLabsOSS/email](https://github.com/SoraLabsOSS/email).

```
POST /api/contact
       ↓
    Queue (email-rust-queue)
       ↓
 Queue Consumer
       ↓
   Resend Email API
```

Rate limiting (Upstash) is not ported yet.

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

Inbox / from-address / CORS live in `wrangler.toml` (`[vars]`).

### 4. Dev

From this repo directory (not `$HOME`):

```sh
npx wrangler dev
```

Wrangler compiles Rust to Wasm via `scripts/build-worker.sh`, then serves the Worker.

```sh
curl http://localhost:8787/health

curl -X POST http://localhost:8787/api/contact \
  -H "Authorization: Bearer <API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{"name":"Jane","email":"jane@example.com","message":"Hi","app":"landing"}'
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

## Deploy

Prefer deploying from a machine that already has Rust (same as the
[Cloudflare Rust guide](https://developers.cloudflare.com/workers/languages/rust/#4-deploy-your-worker-project)):

```sh
npx wrangler queues create email-rust-queue
npx wrangler queues create email-rust-queue-dlq

npx wrangler secret put API_KEYS --name email-rust
npx wrangler secret put RESEND_API_KEY --name email-rust

npx wrangler deploy
```

These queues are **not** the TypeScript worker queues (`email-queue` /
`email-queue-dlq`). Do not point both workers at the same queue.

Git-connected Workers Builds will install Rust in CI when `cargo` is missing.
The first CI build is slow (`cargo install worker-build` compiles from source).
The dashboard deploy command should stay `npx wrangler deploy`.
