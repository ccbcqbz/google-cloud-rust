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
use google_cloud_gax::options::RequestOptionsBuilder;
use google_cloud_storage::client::{Storage, StorageControl};
use google_cloud_wkt::FieldMask;

/// Demonstrates how preconditions and explicit overrides control request idempotency and retries.
///
/// Mutating requests guarded by match preconditions (`if_generation_match` for data/object state,
/// `if_metageneration_match` for metadata updates) or targeting a specific object generation
/// (`generation > 0` on delete) are automatically evaluated as idempotent and retried on transient
/// errors with an `x-goog-gcs-idempotency-token` header.
pub async fn sample(
    client: &Storage,
    control_client: &StorageControl,
    bucket_id: &str,
) -> anyhow::Result<()> {
    let bucket = format!("projects/_/buckets/{bucket_id}");
    let object_name = "sample-idempotent-object.txt";

    // 1. Data-plane upload with `if_generation_match(0)` (create-if-not-exists):
    // Automatically evaluated as idempotent and safe to retry with a deduplication token.
    let created = client
        .write_object(&bucket, object_name, "initial content")
        .set_if_generation_match(0)
        .send_unbuffered()
        .await?;
    println!(
        "Created {} (generation={}, metageneration={})",
        created.name, created.generation, created.metageneration
    );

    // 2. Control-plane metadata update with `if_metageneration_match`:
    // Metadata updates require a metageneration match precondition to be automatically retried.
    let metageneration = created.metageneration;
    let updated_meta = control_client
        .update_object()
        .set_if_metageneration_match(metageneration)
        .set_object(created.set_metadata([("env", "production")]))
        .set_update_mask(FieldMask::default().set_paths(["metadata"]))
        .send()
        .await?;
    println!(
        "Updated metadata for {} (metageneration={})",
        updated_meta.name, updated_meta.metageneration
    );

    // 3. Explicit per-request idempotency override (`.with_idempotency(false)`):
    // Callers can override automatic evaluation on any request builder (via inherent methods on
    // `WriteObject` or `RequestOptionsBuilder::with_idempotency` on `StorageControl` builders)
    // to force single-attempt execution without stamping an idempotency token.
    let overwritten = client
        .write_object(&bucket, object_name, "updated content")
        .set_if_generation_match(updated_meta.generation)
        .with_idempotency(false)
        .send_unbuffered()
        .await?;

    // 4. Generation-specific deletion (`set_generation` > 0):
    // Deleting a specific generation (or setting `if_generation_match`) is idempotent; calling
    // `.with_idempotency(true)` via `RequestOptionsBuilder` is also available when needed.
    control_client
        .delete_object()
        .set_bucket(&bucket)
        .set_object(object_name)
        .set_generation(overwritten.generation)
        .with_idempotency(true)
        .send()
        .await?;
    println!(
        "Deleted {} generation {}",
        object_name, overwritten.generation
    );

    Ok(())
}
// [END storage_configure_idempotency_retry]
