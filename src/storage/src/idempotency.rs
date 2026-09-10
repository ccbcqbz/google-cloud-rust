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

/// HTTP header name used exclusively by Google Cloud Storage for request deduplication across retries.
pub(crate) const IDEMPOTENCY_TOKEN_HEADER: &str = "x-goog-gcs-idempotency-token";

/// Newtype wrapper for request-level GCS idempotency tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IdempotencyToken(pub(crate) String);

impl IdempotencyToken {
    /// Generates a new random UUID v4 idempotency token.
    pub(crate) fn new() -> Self {
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
pub(crate) fn stamp_idempotency_token(
    mut options: google_cloud_gax::options::RequestOptions,
    is_mutating: bool,
) -> google_cloud_gax::options::RequestOptions {
    use google_cloud_gax::options::internal::RequestOptionsExt;

    if is_mutating
        && options.idempotent().unwrap_or(false)
        && options.get_extension::<IdempotencyToken>().is_none()
    {
        let mut headers = options
            .get_extension::<http::HeaderMap>()
            .cloned()
            .unwrap_or_default();

        // Adopt a caller-supplied token when it is usable as a header value, so
        // applications can control the deduplication key. Otherwise mint one.
        let token = headers
            .get(IDEMPOTENCY_TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|value| IdempotencyToken(value.to_string()))
            .unwrap_or_default();

        // Always write the header back. Writing it unconditionally keeps the
        // header on the wire and the `IdempotencyToken` extension in sync even
        // when the pre-existing header value was not valid visible ASCII.
        headers.insert(
            http::header::HeaderName::from_static(IDEMPOTENCY_TOKEN_HEADER),
            http::HeaderValue::from_str(&token.0).expect("valid UUID header"),
        );
        options = options.insert_extension(headers);
        options = options.insert_extension(token);
    }

    options
}

/// Helper function used by request models and handwritten methods to determine effective
/// idempotency and inject the `x-goog-gcs-idempotency-token` header extension when appropriate.
pub(crate) fn configure_idempotency(
    options: google_cloud_gax::options::RequestOptions,
    is_idempotent: bool,
    is_mutating: bool,
) -> google_cloud_gax::options::RequestOptions {
    let options =
        google_cloud_gax::options::internal::set_default_idempotency(options, is_idempotent);
    stamp_idempotency_token(options, is_mutating)
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
        configure_idempotency(options, true, false)
    }
}

impl crate::model::ListObjectsRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, true, false)
    }
}

impl crate::model::GetBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, true, false)
    }
}

impl crate::model::ListBucketsRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, true, false)
    }
}

// 2. Unconditioned Mutating Operations: Non-idempotent by default
impl crate::model::CreateBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, false, true)
    }
}

// 3. Conditional Mutating Operations: Idempotent when match preconditions are present
impl crate::model::LockBucketRetentionPolicyRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_metageneration_match > 0;
        configure_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::DeleteBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent =
            self.if_metageneration_match.is_some() || self.if_metageneration_not_match.is_some();
        configure_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::UpdateBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent =
            self.if_metageneration_match.is_some() || self.if_metageneration_not_match.is_some();
        configure_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::ComposeObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent =
            self.if_generation_match.is_some() || self.if_metageneration_match.is_some();
        configure_idempotency(options, is_idempotent, true)
    }
}

impl crate::model::DeleteObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.generation > 0
            || self.if_generation_match.is_some()
            || self.if_generation_not_match.is_some()
            || self.if_metageneration_match.is_some()
            || self.if_metageneration_not_match.is_some();
        configure_idempotency(options, is_idempotent, true)
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
        configure_idempotency(options, is_idempotent, true)
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
        configure_idempotency(options, is_idempotent, true)
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
        configure_idempotency(options, is_idempotent, true)
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
        configure_idempotency(options, is_idempotent, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_cloud_gax::options::internal::RequestOptionsExt;

    #[test]
    fn test_configure_idempotency_conditionally_safe_mutating() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = configure_idempotency(
            options, true, // is_idempotent
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
    fn test_configure_idempotency_not_safe_mutating() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = configure_idempotency(
            options, false, // is_idempotent
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
        let resolved = configure_idempotency(
            options, false, // is_idempotent is false, but override is true
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
        let resolved2 = configure_idempotency(
            options2, true, // is_idempotent is true, but override is false
            true,
        );
        assert_eq!(resolved2.idempotent(), Some(false));
        assert!(resolved2.get_extension::<IdempotencyToken>().is_none());
        assert!(resolved2.get_extension::<http::HeaderMap>().is_none());
    }

    #[test]
    fn test_stamp_idempotency_token_preserves_existing_token() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let options = google_cloud_gax::options::internal::set_default_idempotency(options, true);
        let options = stamp_idempotency_token(options, true);
        let token1 = options
            .get_extension::<IdempotencyToken>()
            .cloned()
            .expect("token exists");

        // Stamping again on the same options must NOT overwrite the existing token
        let options2 = stamp_idempotency_token(options, true);
        let token2 = options2
            .get_extension::<IdempotencyToken>()
            .cloned()
            .expect("token exists");
        assert_eq!(token1, token2);
    }

    #[test]
    fn test_request_idempotency_evaluations() {
        let is_idempotent = |req_resolved: google_cloud_gax::options::RequestOptions| {
            req_resolved.idempotent() == Some(true)
        };
        let opts = google_cloud_gax::options::RequestOptions::default;

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

        // DeleteObject requires generation > 0 or match preconditions
        assert!(!is_idempotent(
            crate::model::DeleteObjectRequest::default().resolve_idempotency(opts())
        ));
        assert!(is_idempotent(
            crate::model::DeleteObjectRequest {
                generation: 12345,
                ..Default::default()
            }
            .resolve_idempotency(opts())
        ));
        assert!(!is_idempotent(
            crate::model::DeleteObjectRequest {
                generation: 0,
                ..Default::default()
            }
            .resolve_idempotency(opts())
        ));
        assert!(is_idempotent(
            crate::model::DeleteObjectRequest {
                if_generation_match: Some(12345),
                ..Default::default()
            }
            .resolve_idempotency(opts())
        ));

        // DeleteBucket requires metageneration match
        assert!(!is_idempotent(
            crate::model::DeleteBucketRequest::default().resolve_idempotency(opts())
        ));
        assert!(is_idempotent(
            crate::model::DeleteBucketRequest {
                if_metageneration_match: Some(1),
                ..Default::default()
            }
            .resolve_idempotency(opts())
        ));

        // MoveObject requires source or destination generation match
        assert!(!is_idempotent(
            crate::model::MoveObjectRequest::default().resolve_idempotency(opts())
        ));
        assert!(is_idempotent(
            crate::model::MoveObjectRequest {
                if_source_generation_match: Some(54321),
                ..Default::default()
            }
            .resolve_idempotency(opts())
        ));

        // LockBucketRetentionPolicy requires positive metageneration (> 0)
        assert!(!is_idempotent(
            crate::model::LockBucketRetentionPolicyRequest::default().resolve_idempotency(opts())
        ));
        assert!(!is_idempotent(
            crate::model::LockBucketRetentionPolicyRequest {
                if_metageneration_match: -1,
                ..Default::default()
            }
            .resolve_idempotency(opts())
        ));
        assert!(is_idempotent(
            crate::model::LockBucketRetentionPolicyRequest {
                if_metageneration_match: 2,
                ..Default::default()
            }
            .resolve_idempotency(opts())
        ));
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
        let del_cond_req = crate::model::DeleteObjectRequest {
            if_generation_match: Some(100),
            ..Default::default()
        };
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

    #[test]
    fn test_custom_idempotency_token_header_synchronized() {
        let mut custom_headers = http::HeaderMap::new();
        custom_headers.insert(
            http::header::HeaderName::from_static(IDEMPOTENCY_TOKEN_HEADER),
            http::HeaderValue::from_static("custom-uuid-12345"),
        );
        let options =
            google_cloud_gax::options::RequestOptions::default().insert_extension(custom_headers);
        let resolved = configure_idempotency(options, true, true);
        assert_eq!(
            resolved
                .get_extension::<IdempotencyToken>()
                .map(|t| t.0.as_str()),
            Some("custom-uuid-12345")
        );
    }

    // A caller-supplied header value that is not valid visible ASCII cannot be
    // adopted as a token. In that case a fresh token is minted and the header is
    // overwritten, so the extension and the value on the wire always agree.
    #[test]
    fn idempotency_token_extension_matches_header() {
        let mut custom_headers = http::HeaderMap::new();
        custom_headers.insert(
            http::header::HeaderName::from_static(IDEMPOTENCY_TOKEN_HEADER),
            http::HeaderValue::from_bytes(&[0xff, 0xfe]).expect("opaque header value"),
        );
        let options =
            google_cloud_gax::options::RequestOptions::default().insert_extension(custom_headers);
        let resolved = configure_idempotency(options, true, true);

        let extension_token = resolved
            .get_extension::<IdempotencyToken>()
            .map(|t| t.0.clone())
            .expect("token extension exists");
        let wire_header = resolved
            .get_extension::<http::HeaderMap>()
            .and_then(|h| h.get(IDEMPOTENCY_TOKEN_HEADER))
            .map(|v| v.as_bytes().to_vec())
            .expect("header exists");

        assert_eq!(
            extension_token.as_bytes(),
            wire_header.as_slice(),
            "IdempotencyToken extension must match the header actually sent on the wire"
        );
    }

    // Stamping the token rewrites the `HeaderMap` extension. Other headers the
    // caller placed in that map must survive.
    #[test]
    fn stamping_preserves_other_caller_headers() {
        let mut custom_headers = http::HeaderMap::new();
        custom_headers.insert(
            http::header::HeaderName::from_static("x-goog-custom"),
            http::HeaderValue::from_static("keep-me"),
        );
        let options =
            google_cloud_gax::options::RequestOptions::default().insert_extension(custom_headers);
        let resolved = configure_idempotency(options, true, true);

        let headers = resolved
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert_eq!(
            headers.get("x-goog-custom").map(|v| v.as_bytes()),
            Some("keep-me".as_bytes())
        );
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));
    }
}
