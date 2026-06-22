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

// [START storage_application_resumable_upload]
use google_cloud_storage::client::Storage;

pub async fn sample(client: &Storage, bucket: &str, object: &str) -> Result<(), anyhow::Error> {
    let bucket_path = format!("projects/_/buckets/{bucket}");

    // 1. Start a resumable upload session
    let upload_id = client.start_upload(&bucket_path, object).await?;
    println!("Started resumable upload session with ID/URI: {upload_id}");

    // 2. Continue/perform upload using the session
    let payload = "Hello World! This is an application-controlled resumable upload payload.";
    let response = client
        .continue_upload(&bucket_path, object, upload_id, payload)
        .send_buffered()
        .await?;
    println!("Uploaded object: {:?}", response);

    // 3. Start another session and cancel it to demonstrate cancel_resumable_write
    let cancel_object = format!("{}-cancel", object);
    let cancel_upload_id = client.start_upload(&bucket_path, &cancel_object).await?;
    println!("Started another upload session for cancellation: {cancel_upload_id}");

    client
        .cancel_resumable_write(&bucket_path, &cancel_upload_id)
        .await?;
    println!("Successfully cancelled the resumable write.");

    Ok(())
}
// [END storage_application_resumable_upload]
