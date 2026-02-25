# HNDigest Testing Plan

## Phase 1 — Dead Code Identification & Elimination

### 1a. Tooling audit

Run `cargo udeps` (add as a dev tool via `mise`) to find unused crate dependencies. Run `cargo build 2>&1 | grep unused` for unused imports/functions. Also run with `#![warn(dead_code)]` temporarily to surface any unreachable items.

### 1b. Specific suspects to investigate

- **`unsubscribe::lookup_subscriber`** (`src/unsubscribe.rs:9–13`) — a one-line pass-through to `storage.get_subscriber_by_unsubscribe_token`. The only caller is `api.rs:219`. Candidate for inlining and deleting the wrapper.
- **`body_to_string`'s `_ => None` arm** (`src/bin/api.rs:98`) — `lambda_http::Body` is `#[non_exhaustive]`, so this arm may be intentional defensiveness or genuinely unreachable depending on the enum's stability guarantees. Investigate and document or remove.
- **`lib.rs` re-exports** — confirm every module re-exported is consumed by at least one binary; any that aren't should be `pub(crate)` or removed.
- **`post_fetcher`** — verify both the top-K path and the point-threshold path are exercised by `PostSnapshotter::snapshot`. If either fetch strategy is redundant, remove it.

---

## Phase 2 — Refactor for Testability

All business logic currently takes `&Arc<StorageAdapter>` and `Mailer` as concrete types bound to live AWS SDK clients. No tests are possible without real AWS. The fix: **extract thin traits, implemented with hand-written fakes for tests**.

### 2a. Introduce `Storage` trait

In `storage_adapter.rs`, define a `Storage` trait with the same signatures as the current public methods. Rename the existing concrete struct to `LambdaStorage`:

```rust
pub trait Storage: Send + Sync {
    async fn snapshot_posts(...) -> Result<()>;
    async fn save_digest(...) -> Result<()>;
    async fn fetch_digest(...) -> Result<Option<Vec<Post>>>;
    async fn upsert_subscriber(...) -> Result<()>;
    async fn remove_subscriber(...) -> Result<()>;
    async fn get_all_subscribers(...) -> Result<Vec<Subscriber>>;
    async fn get_subscriber_by_email(...) -> Result<Option<Subscriber>>;
    async fn get_subscriber_by_unsubscribe_token(...) -> Result<Option<Subscriber>>;
    async fn upsert_pending_subscription(...) -> Result<()>;
    async fn get_pending_subscription(...) -> Result<Option<PendingSubscription>>;
}
impl Storage for LambdaStorage { /* existing method bodies unchanged */ }
```

### 2b. Introduce `Mailer` trait

Extract a `Mailer` trait with only the single `send_email` method (the common call-site for all three email types). Rename the existing concrete struct to `SesMailer`:

```rust
pub trait Mailer: Send + Sync {
    async fn send_email(
        recipient: &EmailAddress,
        subject: &str,
        html_content: &str,
        text_content: &str,
        extra_headers: &[MessageHeader],
    ) -> Result<()>;
}
impl Mailer for SesMailer { /* existing send_email body unchanged */ }
```

The three higher-level methods (`send_verification_email`, `send_preference_update_email`, `send_digest`) keep their template rendering logic but become free functions that accept `&dyn Mailer` and delegate to `send_email`. This separates template rendering (pure, testable without mocking) from SES request construction (only in `SesMailer`).

### 2c. Introduce `PostFetcher` and `Captcha` traits

Rather than HTTP-mocking the Algolia and Turnstile endpoints, extract traits for these dependencies too:

```rust
// src/post_fetcher.rs
pub trait PostFetcher: Send + Sync {
    async fn fetch_posts(...) -> Result<HashMap<String, Post>>;
}
pub struct AlgoliaPostFetcher { /* existing fields */ }
impl PostFetcher for AlgoliaPostFetcher { /* existing logic */ }

// src/bin/api.rs (or a new captcha.rs)
pub trait Captcha: Send + Sync {
    async fn verify(&self, token: &str) -> Result<bool>;
}
pub struct TurnstileCaptcha { /* http_client + secret_key */ }
impl Captcha for TurnstileCaptcha { /* existing verify_turnstile_token logic */ }
```

`PostSnapshotter` takes `&dyn PostFetcher`; `AppState` holds `Box<dyn Captcha>`.

### 2d. Update call sites

- `subscribe`, `unsubscribe`, `digest_builder` signatures change from `&Arc<StorageAdapter>` to `&Arc<dyn Storage>`
- `AppState` in `api.rs` holds `Arc<dyn Storage>`, `Arc<dyn Mailer>`, `Box<dyn Captcha>`
- Production `main` functions construct `LambdaStorage`, `SesMailer`, `TurnstileCaptcha` and coerce to trait objects as before

### 2e. Hand-written fakes (no mocking library)

With the traits in place, simple concrete fakes in `#[cfg(test)]` modules cover all test needs without pulling in `mockall`:

```rust
struct FakeStorage { subscribers: Mutex<HashMap<...>>, pending: Mutex<HashMap<...>>, ... }
impl Storage for FakeStorage { ... }

struct SpyMailer { sent: Mutex<Vec<SentEmail>> }
impl Mailer for SpyMailer { ... }

struct AlwaysPassCaptcha;
impl Captcha for AlwaysPassCaptcha {
    async fn verify(&self, _token: &str) -> Result<bool> { Ok(true) }
}
```

No dev-dependencies beyond `tokio` (already present) are needed.

---

## Phase 3 — Write Tests

### 3a. Pure unit tests (zero mocking, can start immediately)

| File | Tests |
|---|---|
| `src/types.rs` | `Token::from_str("")` returns error; `Token::generate()` is non-empty; round-trip `Token` through serde |
| `src/types.rs` | `PendingSubscription::new` sets `expires_at = created_at + 24h` |
| `src/strategies.rs` | Already well-covered — extend with `DigestStrategy::select` behavioral tests: TopN returns at most N posts in points order; OverPointThreshold filters correctly |
| `src/digest_builder.rs` | `filter_sent_posts` with no yesterday digest returns all posts; with yesterday digest filters by `object_id`; duplicate IDs across days are excluded |
| `src/storage_adapter.rs` | `subscriber_from_item` and `pending_subscription_from_item` — make `pub(crate)` and test by constructing `HashMap<String, AttributeValue>` directly, verifying DynamoDB serialization without any network call |

### 3b. Fake-based unit tests (require Phase 2)

**`src/subscribe.rs`**
- `create_pending_subscription`: `FakeStorage` → verify a `PendingSubscription` with the correct email/strategy was stored
- `verify_subscription` (valid token): fake holds matching pending → subscriber appears in storage
- `verify_subscription` (wrong token): fake holds mismatched token → no subscriber written, returns `Ok(None)`
- `verify_subscription` (no pending): fake returns nothing → returns `Ok(None)` immediately, no writes
- `update_subscription_strategy`: fake holds existing subscriber → stored subscriber has new strategy, old strategy returned

**`src/unsubscribe.rs`**
- `remove_subscriber` (found): fake holds subscriber for token → subscriber removed from storage, returns `true`
- `remove_subscriber` (not found): fake has no match → nothing removed, returns `false`

**`src/digest_builder.rs`**
- `build_digest` with `TopN(2)`: fake `fetch_digest` returns yesterday's digest with post A; input posts are [A(500pts), B(200pts), C(100pts)]; saved digest is [B, C]; function returns [B, C]
- `build_digest` with `OverPointThreshold(200)`: same setup; result is [B] only
- `build_digest` with no yesterday digest: all posts passed through to strategy selection unchanged

**`src/bin/bounce_handler.rs`**

Extract `handler` body into a testable free function taking `&dyn Storage`:
- Permanent bounce: subscriber removed from storage for each bounced recipient
- Transient bounce: no removals
- Complaint: subscriber removed for each complained recipient
- Invalid email in notification: logged, no storage call, no error propagated

### 3c. API route handler tests (require Phase 2)

`AppState` is parameterised on the traits, so tests construct it with `FakeStorage`, `SpyMailer`, and a `AlwaysPassCaptcha` / `AlwaysFailCaptcha` — no HTTP mocking needed.

| Flow | Scenario | Expected side-effects |
|---|---|---|
| `POST /api/subscribe` | Honeypot field non-empty | No storage/email calls; 200 returned |
| `POST /api/subscribe` | Invalid email | No storage calls; 400 returned |
| `POST /api/subscribe` | Invalid strategy | No storage calls; 400 returned |
| `POST /api/subscribe` | Captcha fails | No storage calls; 400 returned |
| `POST /api/subscribe` | New subscriber (captcha passes) | Pending subscription stored; verification email sent with URL containing token; 200 |
| `POST /api/subscribe` | Existing subscriber, new strategy | Subscriber updated with new strategy; preference update email sent; 200 |
| `GET /api/verify` | Valid email + matching token | Subscriber written to storage; redirect to `/verify-success.html` |
| `GET /api/verify` | Token mismatch | No subscriber written; redirect to `/verify-error.html` |
| `GET /api/unsubscribe` | Valid token | Renders HTML confirmation page containing subscriber's email |
| `GET /api/unsubscribe` | Unknown token | Redirect to `/unsubscribe-error.html` |
| `POST /api/unsubscribe` (browser) | Valid token | Subscriber removed; redirect to `/unsubscribe-success.html` |
| `POST /api/unsubscribe` (RFC 8058) | Valid token + body `List-Unsubscribe=One-Click` | Subscriber removed; 200 plain text response |
| `POST /api/unsubscribe` (browser) | Unknown token | Nothing removed; redirect to `/unsubscribe-error.html` |

---

## Phase 4 — Coverage Measurement

Add `cargo-llvm-cov` as a dev tool (via `mise`) for a one-time coverage report once tests are written:

```bash
cargo llvm-cov --all-features --workspace
```

No enforcement threshold — use it to spot gaps rather than gate CI.

---

## Execution Order

```
Phase 1            dead code cleanup (unblocks cleaner diffs in Phase 2)
Phase 3a           pure unit tests — can start immediately, in parallel with Phase 2
Phase 2a           Storage trait + LambdaStorage rename — enables Phase 3b + 3c
Phase 2b           Mailer trait + SesMailer rename — enables Phase 3c
Phase 2c           PostFetcher + Captcha traits — enables Phase 3c fully
Phase 2d/e         update call sites + write fakes
Phase 3b           unit tests for subscribe, unsubscribe, digest_builder, bounce handler
Phase 3c           API route tests
Phase 4            coverage snapshot
```

The biggest leverage is Phase 2 — it unlocks almost all meaningful behavioral tests without requiring any AWS infrastructure.
