use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a GCS bucket notification configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Notification {
    /// The GCS-assigned unique identifier for the notification configuration.
    pub id: String,
    /// The Cloud Pub/Sub topic to which notifications are published.
    /// Format: `//pubsub.googleapis.com/projects/{project-id}/topics/{topic-name}`
    pub topic: String,
    /// The list of event types that trigger a notification.
    /// Supported values include: `OBJECT_FINALIZE`, `OBJECT_METADATA_UPDATE`, `OBJECT_DELETE`, `OBJECT_ARCHIVE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_types: Option<Vec<String>>,
    /// Custom attributes attached to the Pub/Sub message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, String>>,
    /// Filter notifications by object name prefix (e.g., `uploads/`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_name_prefix: Option<String>,
    /// The payload format of the Pub/Sub message.
    /// Supported values include: `JSON_API_V1` (JSON representation of object metadata) or `NONE` (no payload).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_format: Option<String>,
}

/// Options for creating a new bucket notification configuration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CreateNotificationOptions {
    /// The list of event types that trigger a notification.
    /// Supported values include: `OBJECT_FINALIZE`, `OBJECT_METADATA_UPDATE`, `OBJECT_DELETE`, `OBJECT_ARCHIVE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_types: Option<Vec<String>>,
    /// Custom attributes attached to the Pub/Sub message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, String>>,
    /// Filter notifications by object name prefix (e.g., `uploads/`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_name_prefix: Option<String>,
    /// The payload format of the Pub/Sub message.
    /// Supported values include: `JSON_API_V1` (JSON representation of object metadata) or `NONE` (no payload).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_format: Option<String>,
}

/// Internal helper struct for the POST request body when creating a notification.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateNotificationRequest<'a> {
    pub topic: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_name_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_format: Option<String>,
}

/// Internal helper struct for deserializing the list notifications response.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ListNotificationsResponse {
    pub items: Option<Vec<Notification>>,
}
