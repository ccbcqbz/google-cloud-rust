use google_cloud_storage::client::Storage;
use google_cloud_storage::notification::CreateNotificationOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Storage::builder().build().await?;
    let bucket = "my-gcs-bucket";
    let topic = "//pubsub.googleapis.com/projects/my-project/topics/my-topic";

    // 1. Create Notification
    let mut options = CreateNotificationOptions::default();
    options.payload_format = Some("JSON_API_V1".to_string());
    options.event_types = Some(vec!["OBJECT_FINALIZE".to_string()]);

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
