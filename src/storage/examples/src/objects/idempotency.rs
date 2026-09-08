// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// [START storage_configure_idempotency_retry]
use google_cloud_storage::client::{Storage, StorageControl};

/// Demonstrates configuring idempotency and match preconditions in Google Cloud Storage.
///
/// Mutating requests guarded by match preconditions (e.g. `if_generation_match`) are
/// automatically marked as idempotent and safe to retry under transient errors (HTTP 503, 429).
/// The client library automatically stamps the `x-goog-gcs-idempotency-token` header
/// for backend request deduplication across retry attempts.
pub async fn sample(
    client: &Storage,
    control_client: &StorageControl,
    bucket_id: &str,
) -> anyhow::Result<()> {
    let bucket = format!("projects/_/buckets/{bucket_id}");
    let object_name = "sample-idempotent-object.txt";
    let data = bytes::Bytes::from("Hello, GCS Idempotency Parity!");

    // 1. Single-shot upload with match precondition:
    // `set_if_generation_match(0)` asserts that the object must not already exist.
    // The SDK automatically marks this mutating request as idempotent, enabling
    // automatic retry with the `x-goog-gcs-idempotency-token` deduplication header.
    let created = client
        .write_object(&bucket, object_name, data)
        .set_if_generation_match(0)
        .send_unbuffered()
        .await?;
    println!(
        "Created object {} with generation {}",
        created.name, created.generation
    );

    // 2. Mutating request with explicit manual override:
    // Callers can explicitly force or disable retries via `.with_idempotency(bool)`.
    // Setting `with_idempotency(false)` disables retries even if preconditions are present.
    let updated_data = bytes::Bytes::from("Updated content");
    let updated = client
        .write_object(&bucket, object_name, updated_data)
        .set_if_generation_match(created.generation)
        .with_idempotency(true) // Explicit override
        .send_unbuffered()
        .await?;
    println!(
        "Updated object {} generation: {}",
        updated.name, updated.generation
    );

    // 3. Clean up the sample object:
    // Deleting with precondition is safe to retry on network drops.
    control_client
        .delete_object()
        .set_bucket(&bucket)
        .set_object(object_name)
        .set_if_generation_match(updated.generation)
        .send()
        .await?;
    println!("Successfully deleted object with precondition");

    Ok(())
}
// [END storage_configure_idempotency_retry]
