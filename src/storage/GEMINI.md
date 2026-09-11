# Google Cloud Client Libraries for Rust - Storage

## Project Overview

This directory contains the `google-cloud-storage` Rust crate, which provides
idiomatic client libraries to interact with Google Cloud Storage. It is part of
the larger `google-cloud-rust` workspace.

The library exposes two primary clients:

- **`Storage`** (`src/storage/client.rs`): Handles data plane operations
  (reading, writing, and managing objects).
- **`StorageControl`** (`src/control/client.rs`): Handles control plane and
  administrative operations (managing buckets, folders, and managed folders).

Most of the underlying communication logic, gRPC/Protobuf representations, and
base clients are generated (located in `src/generated` and `src/google`), while
ergonomic wrappers, trait implementations, and custom logic (like
`read_resume_policy` and `streaming_source`) are manually maintained.

## Key Directories and Files

- **`src/`**: The core library code.
  - `src/lib.rs`: The root of the crate, exporting the main clients, builders,
    stubs, and types.
  - `src/idempotency.rs`: Request-level idempotency resolution and
    `x-goog-gcs-idempotency-token` deduplication header stamping for
    `google.storage.v2` RPCs and handwritten uploads.
  - `src/storage/`: Implementations for the data plane client (bidi streaming,
    open object, read/write object).
  - `src/control/`: Implementations for the control plane client.
  - `src/generated/` & `src/google/`: Auto-generated protobuf code (do not edit
    directly).
  - `src/stub/`: Defines traits and structures for mocking client interactions
    during tests.
- **`tests/`**: Contains unit and integration tests (e.g., `mocking.rs`,
  `binding.rs`, `grpc_mock_idempotency.rs`).
  - **`tests/scenarios/`**: A standalone binary package (`storage-scenarios`)
    used for stress testing bidirectional streaming reads against live GCP
    environments.
- **`examples/`**: Code examples showcasing how to authenticate and use the
  library.
- **`benchmarks/`**: Contains benchmarking tools to measure client performance.
- **`grpc-mock/`**: Provides a mock gRPC server for testing the client locally
  without requiring live GCP access.

## Building and Running

Since this crate is part of a larger Cargo workspace, standard Cargo commands
are used. Run these commands from within the `storage` directory or the
workspace root.

**Building:**

```bash
cargo build --package google-cloud-storage
```

**Testing:**

```bash
# Run tests for this crate
cargo test --package google-cloud-storage

# Run tests with the unstable-stream feature enabled
cargo test --package google-cloud-storage --features unstable-stream
```

**Running Scenarios (Stress Tests):** The stress testing utility is built as a
separate package:

```bash
cargo run --release --package storage-scenarios -- --bucket-name <BUCKET_NAME> [OPTIONS]
```

## Development Conventions

- **Asynchronous Execution:** The library is heavily reliant on `tokio` and
  `futures`. All RPCs are asynchronous.
- **Code Generation:** Core protobuf definitions are generated from Google API
  descriptors using the internal `sidekick` tool. Manual implementation work
  should occur in extension files like `model_ext.rs`, `builder_ext.rs`,
  `idempotency.rs`, or the high-level `client.rs` implementations rather than
  the generated modules.
- **Idempotency & Retries (Adding New APIs):**
  - **Generated `google.storage.v2.Storage` RPCs (`src/generated/gapic/`):**
    Configured with `idempotency_hook: resolve_idempotency` in the root
    `librarian.yaml`. When adding a new unary RPC to `google.storage.v2`:
    1. Implement `pub(crate) fn resolve_idempotency(&self, options: RequestOptions) -> RequestOptions`
       on the request struct in `src/idempotency.rs`, delegating to
       `configure_idempotency(options, Operation::Read)` or
       `configure_idempotency(options, Operation::mutation(...))` per the
       [GCS retry strategy](https://cloud.google.com/storage/docs/retry-strategy#idempotency-operations)
       (`*_not_match` preconditions must **never** be treated as idempotent).
    2. Update the expected RPC count in the `gapic_transport_idempotency_hook_integrity`
       test in `src/idempotency.rs` and add unit test cases for the new request.
  - **Handwritten Data-Plane Operations (`src/storage/perform_upload/`, etc.):**
    Call `crate::idempotency::configure_idempotency` **once outside the retry
    loop** so that `x-goog-gcs-idempotency-token` is generated once per logical
    request and reused unchanged across all retry attempts.
  - **Generated `google.storage.control.v2.StorageControl` RPCs (`src/generated/gapic_control/`):**
    Do **not** use `idempotency_hook`. `storage_control.proto` includes
    `google.api.http` annotations and AIP-155 `request_id` fields, so `sidekick`
    emits static `set_default_idempotency` calls automatically.
- **Mocking Strategy:** The library provides robust mocking capabilities for
  developers using the crate. The `src/stub/` module defines traits that can be
  implemented or mocked using `mockall` to simulate GCP behavior in unit tests.
- **Networking & Crypto:** The `default-rustls-provider` feature is enabled by
  default, using `aws-lc-rs` for TLS.
- **Configuration Defaults:** The `Storage` client provides a `builder()`
  pattern (`ClientBuilder`) to configure options like `with_endpoint`,
  `with_credentials`, `with_retry_policy`, `with_grpc_subchannel_count`, and
  `with_tracing`.
- **Linting & Formatting:** Ensure code complies with workspace standards:
  ```bash
  cargo clippy --package google-cloud-storage
  cargo fmt --package google-cloud-storage
  ```
