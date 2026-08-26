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

//! Types and utilities for configuring idempotency and retry safety in Google Cloud Storage.

/// Configures how the client evaluates whether an operation is idempotent and safe to retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum IdempotencyPolicy {
    /// Safe default mode.
    ///
    /// Read-only operations (such as `read_object`, `get_bucket`, `list_objects`) are always
    /// considered idempotent. Mutating operations (such as `write_object`, `delete_object`,
    /// `move_object`) are considered idempotent only when guarded by required request
    /// preconditions (such as `if_generation_match` or `if_metageneration_match`).
    #[default]
    RetryIdempotent,

    /// Retries all operations regardless of idempotency or preconditions.
    ///
    /// In this mode, even mutating requests without preconditions are treated as idempotent
    /// and eligible for retries on transient errors.
    RetryAlways,

    /// Never retries any operations, regardless of idempotency or preconditions.
    ///
    /// In this mode, operations are attempted exactly once and will not retry on errors.
    RetryNever,
}

/// HTTP header name used exclusively by Google Cloud Storage for request deduplication across retries.
pub const IDEMPOTENCY_TOKEN_HEADER: &str = "x-goog-gcs-idempotency-token";

/// Newtype wrapper for request-level GCS idempotency tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdempotencyToken(pub String);

impl IdempotencyToken {
    /// Generates a new random UUID v4 idempotency token.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for IdempotencyToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Stamps an `x-goog-gcs-idempotency-token` header extension into `RequestOptions`
/// if the operation is mutating, evaluated as idempotent, and no token is already present.
pub fn stamp_idempotency_token(
    mut options: google_cloud_gax::options::RequestOptions,
    is_mutating: bool,
) -> google_cloud_gax::options::RequestOptions {
    use google_cloud_gax::options::internal::RequestOptionsExt;

    if is_mutating
        && options.idempotent().unwrap_or(false)
        && options.get_extension::<IdempotencyToken>().is_none()
    {
        let token = IdempotencyToken::new();
        let mut headers = options
            .get_extension::<http::HeaderMap>()
            .cloned()
            .unwrap_or_default();

        if !headers.contains_key(IDEMPOTENCY_TOKEN_HEADER) {
            headers.insert(
                http::header::HeaderName::from_static(IDEMPOTENCY_TOKEN_HEADER),
                http::HeaderValue::from_str(&token.0).expect("valid UUID header"),
            );
            options = options.insert_extension(headers);
        }
        options = options.insert_extension(token);
    }

    options
}

/// Helper function used by generated stubs and handwritten methods to determine effective
/// idempotency and inject the `x-goog-gcs-idempotency-token` header extension when appropriate.
pub fn resolve_idempotency(
    mut options: google_cloud_gax::options::RequestOptions,
    client_policy: IdempotencyPolicy,
    is_conditionally_safe: bool,
    is_mutating: bool,
) -> google_cloud_gax::options::RequestOptions {
    // 1. If per-request idempotency was explicitly configured, it overrides the client policy.
    let effective_idempotent = match options.idempotent() {
        Some(explicit) => explicit,
        None => match client_policy {
            IdempotencyPolicy::RetryIdempotent => is_conditionally_safe,
            IdempotencyPolicy::RetryAlways => true,
            IdempotencyPolicy::RetryNever => false,
        },
    };

    // 2. Set the effective idempotency on the RequestOptions.
    options.set_idempotency(effective_idempotent);

    // 3. Stamp token if mutating and idempotent.
    stamp_idempotency_token(options, is_mutating)
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_cloud_gax::options::internal::RequestOptionsExt;

    #[test]
    fn test_retry_idempotent_with_preconditions() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = resolve_idempotency(
            options,
            IdempotencyPolicy::RetryIdempotent,
            true, // is_conditionally_safe
            true, // is_mutating
        );
        assert_eq!(resolved.idempotent(), Some(true));
        assert!(resolved.get_extension::<IdempotencyToken>().is_some());
        let headers = resolved
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));
    }

    #[test]
    fn test_retry_idempotent_without_preconditions() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = resolve_idempotency(
            options,
            IdempotencyPolicy::RetryIdempotent,
            false, // is_conditionally_safe
            true,  // is_mutating
        );
        assert_eq!(resolved.idempotent(), Some(false));
        assert!(resolved.get_extension::<IdempotencyToken>().is_none());
        assert!(resolved.get_extension::<http::HeaderMap>().is_none());
    }

    #[test]
    fn test_retry_always() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = resolve_idempotency(
            options,
            IdempotencyPolicy::RetryAlways,
            false, // is_conditionally_safe
            true,  // is_mutating
        );
        assert_eq!(resolved.idempotent(), Some(true));
        assert!(resolved.get_extension::<IdempotencyToken>().is_some());
        let headers = resolved
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));
    }

    #[test]
    fn test_retry_never() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = resolve_idempotency(
            options,
            IdempotencyPolicy::RetryNever,
            true, // is_conditionally_safe
            true, // is_mutating
        );
        assert_eq!(resolved.idempotent(), Some(false));
        assert!(resolved.get_extension::<IdempotencyToken>().is_none());
        assert!(resolved.get_extension::<http::HeaderMap>().is_none());
    }

    #[test]
    fn test_per_request_override_takes_precedence() {
        let mut options = google_cloud_gax::options::RequestOptions::default();
        options.set_idempotency(true);
        let resolved = resolve_idempotency(
            options,
            IdempotencyPolicy::RetryNever, // client says never
            false,
            true,
        );
        assert_eq!(resolved.idempotent(), Some(true));
        assert!(resolved.get_extension::<IdempotencyToken>().is_some());
        let headers = resolved
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));
    }

    #[test]
    fn test_read_only_operation_no_mutating_token() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = resolve_idempotency(
            options,
            IdempotencyPolicy::RetryIdempotent,
            true,  // read-only operations are inherently safe
            false, // is_mutating = false
        );
        assert_eq!(resolved.idempotent(), Some(true));
        assert!(resolved.get_extension::<IdempotencyToken>().is_none());
        assert!(resolved.get_extension::<http::HeaderMap>().is_none());
    }
}
