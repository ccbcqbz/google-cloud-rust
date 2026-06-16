//! Types and constants for GCS Bucket Pub/Sub Notifications.
//!
//! GCS Bucket Notifications allow applications to receive events (such as object creation or deletion)
//! asynchronously via Google Cloud Pub/Sub.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Send object metadata as JSON with notification messages.
pub const PAYLOAD_FORMAT_JSON_API_V1: &str = "JSON_API_V1";
/// Send no payload with notification messages.
pub const PAYLOAD_FORMAT_NONE: &str = "NONE";

/// Event that occurs when an object is successfully created or overwritten.
pub const EVENT_OBJECT_FINALIZE: &str = "OBJECT_FINALIZE";
/// Event that occurs when the metadata of an existing object changes.
pub const EVENT_OBJECT_METADATA_UPDATE: &str = "OBJECT_METADATA_UPDATE";
/// Event that occurs when an object is permanently deleted.
pub const EVENT_OBJECT_DELETE: &str = "OBJECT_DELETE";
/// Event that occurs when the live version of an object becomes an archived version.
pub const EVENT_OBJECT_ARCHIVE: &str = "OBJECT_ARCHIVE";

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
    /// The kind of item. Always `storage#notification`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The link to this resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_link: Option<String>,
    /// The list of event types that trigger a notification.
    /// Supported values include `EVENT_OBJECT_FINALIZE`, `EVENT_OBJECT_METADATA_UPDATE`,
    /// `EVENT_OBJECT_DELETE`, and `EVENT_OBJECT_ARCHIVE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_types: Option<Vec<String>>,
    /// Custom attributes attached to the Pub/Sub message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, String>>,
    /// Filter notifications by object name prefix (e.g., `uploads/`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_name_prefix: Option<String>,
    /// The payload format of the Pub/Sub message.
    /// Supported values include `PAYLOAD_FORMAT_JSON_API_V1` or `PAYLOAD_FORMAT_NONE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_format: Option<String>,
}

/// Options for creating a new bucket notification configuration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CreateNotificationOptions {
    /// The list of event types that trigger a notification.
    /// Supported values include `EVENT_OBJECT_FINALIZE`, `EVENT_OBJECT_METADATA_UPDATE`,
    /// `EVENT_OBJECT_DELETE`, and `EVENT_OBJECT_ARCHIVE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_types: Option<Vec<String>>,
    /// Custom attributes attached to the Pub/Sub message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, String>>,
    /// Filter notifications by object name prefix (e.g., `uploads/`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_name_prefix: Option<String>,
    /// The payload format of the Pub/Sub message.
    /// Supported values include `PAYLOAD_FORMAT_JSON_API_V1` or `PAYLOAD_FORMAT_NONE`.
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ListNotificationsResponse {
    pub items: Option<Vec<Notification>>,
}

