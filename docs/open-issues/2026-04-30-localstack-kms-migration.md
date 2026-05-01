# test(storage): remove `#[ignore]` from `kms::aws::tests::live_roundtrip` via LocalStack KMS

> **Status:** drafted 2026-04-30 in the v1.5.0 contract-correctness sweep, awaiting filing as a GitHub issue. Once filed, replace the placeholder reference in `storage/src/kms/aws.rs` with the issue URL and delete this file.

## Problem

`storage/src/kms/aws.rs::tests::live_roundtrip` is the only `#[ignore]`-marked test in the storage crate. It exercises the real AWS KMS round-trip (`AwsKmsProvider::new` → `encrypt` → `decrypt`) and is gated behind `AETERNA_KMS_LIVE_TEST_ARN` because it makes billable network calls against a real AWS account.

```rust
// storage/src/kms/aws.rs
#[tokio::test]
#[ignore = "requires live AWS credentials + AETERNA_KMS_LIVE_TEST_ARN; see docs/open-issues/2026-04-30-localstack-kms-migration.md"]
async fn live_roundtrip() {
    let arn = std::env::var("AETERNA_KMS_LIVE_TEST_ARN")
        .expect("AETERNA_KMS_LIVE_TEST_ARN must be set for live test");
    let kms = AwsKmsProvider::new(&arn).await.unwrap();
    // … real KMS encrypt/decrypt round-trip
}
```

The v1.5.0 sweep raised the bar to **no ignored tests in tree without an explicit tracker**. This document captures the work to remove the ignore.

## Why it's currently ignored (acceptable for now)

- Running it in CI without credentials would fail with "no credentials" noise on every PR.
- Running it in CI **with** credentials would bill the AWS account on every PR push and would require a long-lived test KMS key in the kyriba sandbox account.
- The `AwsKmsProvider` constructor + error paths are exercised by non-ignored unit tests in the same file (`rejects_empty_key_arn`, `rejects_whitespace_key_arn`) and by the trait-level production gate matrix in `storage::secret_backend::gate_tests` (introduced in A4).

So the production code path **is** under regression coverage today. The `live_roundtrip` test is the icing — a real-cloud smoke check.

## Proposed solution: LocalStack KMS

Migrate the test off real AWS and onto [LocalStack's KMS emulator](https://docs.localstack.cloud/aws/services/kms/), then drop the `#[ignore]`. LocalStack is already used in the docker-compose dev stack for S3 in some adjacent tests; adding the KMS service is a one-line addition.

### Implementation sketch

1. **`docker-compose.test.yml`** — add `kms` to LocalStack's `SERVICES` env var.
2. **`storage/src/kms/aws.rs`** — confirm `AwsKmsProvider::new` honors the standard `AWS_ENDPOINT_URL` env var so the test can point the SDK at `http://localhost:4566`. (The AWS SDK already picks this up from env; verify our config builder doesn't hard-code an endpoint.)
3. **Test setup** — `#[serial]`-mark the test, create a fresh KMS key on each run via the AWS SDK against the LocalStack endpoint, drop the `#[ignore]`. Test runs unconditionally on every PR.
4. **CI** — the existing PR Integration workflow already spins up LocalStack; just confirm the new service starts.

## Alternative (smaller): strengthen non-ignored coverage

Instead of LocalStack, add a non-ignored `endpoint_unreachable_returns_clean_connection_error` test that points the SDK at `http://127.0.0.1:0` and asserts the error variant. Leaves `live_roundtrip` ignored as a manual smoke ping.

Faster to ship, but doesn't honor the "no ignored tests" principle.

## Acceptance criteria

- [ ] `cargo test -p storage --lib --` lists 0 ignored tests.
- [ ] `live_roundtrip` (or its replacement) runs unconditionally on every PR without requiring live AWS credentials.
- [ ] Production `AwsKmsProvider` behavior is unchanged — the test path uses LocalStack only when `AWS_ENDPOINT_URL` is set, which is never the case in production.
- [ ] CI runtime delta < 5s.

## Context

Surfaced during the v1.5.0 contract-correctness sweep (rc.9 fix-pack PR). The user's "never ignore tests" principle made the ignore a debt item that needs an explicit tracker rather than a silent acceptance.

Suggested labels: `test`, `tech-debt`, `storage`, `kms`, `good-first-issue`
