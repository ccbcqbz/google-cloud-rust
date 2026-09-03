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

/// Helper function used by request models and handwritten methods to determine effective
/// idempotency and inject the `x-goog-gcs-idempotency-token` header extension when appropriate.
pub fn resolve_idempotency(
    options: google_cloud_gax::options::RequestOptions,
    is_conditionally_safe: bool,
    is_mutating: bool,
) -> google_cloud_gax::options::RequestOptions {
    let options = google_cloud_gax::options::internal::set_default_idempotency(
        options,
        is_conditionally_safe,
    );
    stamp_idempotency_token(options, is_mutating)
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_cloud_gax::options::internal::RequestOptionsExt;

    #[test]
    fn test_resolve_idempotency_conditionally_safe_mutating() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = resolve_idempotency(
            options, true, // is_conditionally_safe
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
    fn test_resolve_idempotency_not_safe_mutating() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = resolve_idempotency(
            options, false, // is_conditionally_safe
            true,  // is_mutating
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
            options, false, // is_conditionally_safe is false, but override is true
            true,
        );
        assert_eq!(resolved.idempotent(), Some(true));
        assert!(resolved.get_extension::<IdempotencyToken>().is_some());
        let headers = resolved
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));

        let mut options2 = google_cloud_gax::options::RequestOptions::default();
        options2.set_idempotency(false);
        let resolved2 = resolve_idempotency(
            options2, true, // is_conditionally_safe is true, but override is false
            true,
        );
        assert_eq!(resolved2.idempotent(), Some(false));
        assert!(resolved2.get_extension::<IdempotencyToken>().is_none());
        assert!(resolved2.get_extension::<http::HeaderMap>().is_none());
    }

    #[test]
    fn test_read_only_operation_no_mutating_token() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = resolve_idempotency(
            options, true,  // read-only operations are inherently safe
            false, // is_mutating = false
        );
        assert_eq!(resolved.idempotent(), Some(true));
        assert!(resolved.get_extension::<IdempotencyToken>().is_none());
        assert!(resolved.get_extension::<http::HeaderMap>().is_none());
    }

    #[test]
    fn test_request_idempotency_evaluations() {
        let is_idempotent = |req_resolved: google_cloud_gax::options::RequestOptions| {
            req_resolved.idempotent() == Some(true)
        };
        let opts = || google_cloud_gax::options::RequestOptions::default();

        // Reads are always idempotent
        assert!(is_idempotent(
            crate::model::GetObjectRequest::default().resolve_idempotency(opts())
        ));
        assert!(is_idempotent(
            crate::model::ListObjectsRequest::default().resolve_idempotency(opts())
        ));
        assert!(is_idempotent(
            crate::model::GetBucketRequest::default().resolve_idempotency(opts())
        ));
        assert!(is_idempotent(
            crate::model::ListBucketsRequest::default().resolve_idempotency(opts())
        ));

        // CreateBucket is never idempotent by default
        assert!(!is_idempotent(
            crate::model::CreateBucketRequest::default().resolve_idempotency(opts())
        ));

        // DeleteObject requires match preconditions
        let mut del_obj = crate::model::DeleteObjectRequest::default();
        assert!(!is_idempotent(del_obj.resolve_idempotency(opts())));
        del_obj.if_generation_match = Some(12345);
        assert!(is_idempotent(del_obj.resolve_idempotency(opts())));

        // DeleteBucket requires metageneration match
        let mut del_bkt = crate::model::DeleteBucketRequest::default();
        assert!(!is_idempotent(del_bkt.resolve_idempotency(opts())));
        del_bkt.if_metageneration_match = Some(1);
        assert!(is_idempotent(del_bkt.resolve_idempotency(opts())));

        // MoveObject requires source or destination generation match
        let mut move_obj = crate::model::MoveObjectRequest::default();
        assert!(!is_idempotent(move_obj.resolve_idempotency(opts())));
        move_obj.if_source_generation_match = Some(54321);
        assert!(is_idempotent(move_obj.resolve_idempotency(opts())));

        // LockBucketRetentionPolicy requires positive metageneration (> 0)
        let mut lock_bkt = crate::model::LockBucketRetentionPolicyRequest::default();
        assert!(!is_idempotent(lock_bkt.resolve_idempotency(opts())));
        lock_bkt.if_metageneration_match = -1;
        assert!(!is_idempotent(lock_bkt.resolve_idempotency(opts())));
        lock_bkt.if_metageneration_match = 2;
        assert!(is_idempotent(lock_bkt.resolve_idempotency(opts())));
    }

    #[test]
    fn test_request_resolve_idempotency() {
        // 1. Read request: resolve_idempotency sets idempotent=true, does not stamp tokens
        let get_req = crate::model::GetObjectRequest::default();
        let options = google_cloud_gax::options::RequestOptions::default();
        let options = get_req.resolve_idempotency(options);
        assert_eq!(options.idempotent(), Some(true));
        assert!(options.get_extension::<IdempotencyToken>().is_none());
        assert!(options.get_extension::<http::HeaderMap>().is_none());

        // 2. Unconditioned mutating request: not idempotent, no token stamped
        let del_req = crate::model::DeleteObjectRequest::default();
        let options = google_cloud_gax::options::RequestOptions::default();
        let options = del_req.resolve_idempotency(options);
        assert_eq!(options.idempotent(), Some(false));
        assert!(options.get_extension::<IdempotencyToken>().is_none());
        assert!(options.get_extension::<http::HeaderMap>().is_none());

        // 3. Conditioned mutating request: idempotent, token stamped into header and extension
        let mut del_cond_req = crate::model::DeleteObjectRequest::default();
        del_cond_req.if_generation_match = Some(100);
        let options = google_cloud_gax::options::RequestOptions::default();
        let options = del_cond_req.resolve_idempotency(options);
        assert_eq!(options.idempotent(), Some(true));
        assert!(options.get_extension::<IdempotencyToken>().is_some());
        let headers = options
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));

        // 4. Overridden mutating request: explicit idempotency true stamps token
        let create_req = crate::model::CreateBucketRequest::default();
        let mut options = google_cloud_gax::options::RequestOptions::default();
        options.set_idempotency(true);
        let options = create_req.resolve_idempotency(options);
        assert_eq!(options.idempotent(), Some(true));
        assert!(options.get_extension::<IdempotencyToken>().is_some());
        let headers = options
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));
    }
}

// -----------------------------------------------------------------------------------------
// GCS Request-Level Idempotency Evaluations
// -----------------------------------------------------------------------------------------

// 1. Read / List Operations: Inherently idempotent
impl crate::model::GetObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        resolve_idempotency(options, true, false)
    }
}

impl crate::model::ListObjectsRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        resolve_idempotency(options, true, false)
    }
}

impl crate::model::GetBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        resolve_idempotency(options, true, false)
    }
}

impl crate::model::ListBucketsRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        resolve_idempotency(options, true, false)
    }
}

// 2. Unconditioned Mutating Operations: Non-idempotent by default
impl crate::model::CreateBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        resolve_idempotency(options, false, true)
    }
}

// 3. Conditional Mutating Operations: Idempotent when match preconditions are present
impl crate::model::LockBucketRetentionPolicyRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_metageneration_match > 0;
        resolve_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::DeleteBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent =
            self.if_metageneration_match.is_some() || self.if_metageneration_not_match.is_some();
        resolve_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::UpdateBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent =
            self.if_metageneration_match.is_some() || self.if_metageneration_not_match.is_some();
        resolve_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::ComposeObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent =
            self.if_generation_match.is_some() || self.if_metageneration_match.is_some();
        resolve_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::DeleteObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_generation_match.is_some()
            || self.if_generation_not_match.is_some()
            || self.if_metageneration_match.is_some()
            || self.if_metageneration_not_match.is_some();
        resolve_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::RestoreObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_generation_match.is_some()
            || self.if_generation_not_match.is_some()
            || self.if_metageneration_match.is_some()
            || self.if_metageneration_not_match.is_some();
        resolve_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::UpdateObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_generation_match.is_some()
            || self.if_generation_not_match.is_some()
            || self.if_metageneration_match.is_some()
            || self.if_metageneration_not_match.is_some();
        resolve_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::RewriteObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_generation_match.is_some()
            || self.if_generation_not_match.is_some()
            || self.if_metageneration_match.is_some()
            || self.if_metageneration_not_match.is_some()
            || self.if_source_generation_match.is_some()
            || self.if_source_generation_not_match.is_some()
            || self.if_source_metageneration_match.is_some()
            || self.if_source_metageneration_not_match.is_some();
        resolve_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::MoveObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_source_generation_match.is_some()
            || self.if_source_generation_not_match.is_some()
            || self.if_source_metageneration_match.is_some()
            || self.if_source_metageneration_not_match.is_some()
            || self.if_generation_match.is_some()
            || self.if_generation_not_match.is_some()
            || self.if_metageneration_match.is_some()
            || self.if_metageneration_not_match.is_some();
        resolve_idempotency(options, is_idempotent, true)
    }
}
