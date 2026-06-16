// Copyright 2025 Google LLC
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

use google_cloud_storage::client::Storage;
use google_cloud_storage::notification::{
    CreateNotificationOptions, EVENT_OBJECT_FINALIZE, PAYLOAD_FORMAT_JSON_API_V1,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Storage::builder().build().await?;
    let bucket = "projects/_/buckets/my-gcs-bucket";
    let topic = "//pubsub.googleapis.com/projects/my-project/topics/my-topic";

    // 1. Create Notification
    let mut options = CreateNotificationOptions::default();
    options.payload_format = Some(PAYLOAD_FORMAT_JSON_API_V1.to_string());
    options.event_types = Some(vec![EVENT_OBJECT_FINALIZE.to_string()]);

    let created = client.create_notification(bucket, topic, options).await?;
    println!("Created notification config: {:?}", created);

    // 2. Get Notification
    let fetched = client.get_notification(bucket, &created.id).await?;
    println!("Fetched notification config: {:?}", fetched);

    // 3. List Notifications
    let notifications = client.list_notifications(bucket).await?;
    println!("List notifications: {:?}", notifications);

    // 4. Delete Notification
    client.delete_notification(bucket, &created.id).await?;
    println!("Deleted notification config {}", created.id);

    Ok(())
}
