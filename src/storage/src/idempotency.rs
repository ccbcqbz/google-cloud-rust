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

//! Types and utilities for configuring idempotency and retry safety in Google Cloud Storage.

/// HTTP header name used exclusively by Google Cloud Storage for request deduplication across retries.
pub(crate) const IDEMPOTENCY_TOKEN_HEADER: &str = "x-goog-gcs-idempotency-token";

/// Classification of a GCS operation for idempotency and retry configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    /// Reads and lists: always retryable, never carry a dedup token.
    Read,
    /// Mutations: retryable only when `idempotent`, and carry a dedup token when idempotent.
    Mutation { idempotent: bool },
}

/// Newtype wrapper for request-level GCS idempotency tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IdempotencyToken(pub(crate) String);

impl IdempotencyToken {
    /// Generates a new random UUID v4 idempotency token.
    pub(crate) fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
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

        match headers
            .get(IDEMPOTENCY_TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
        {
            // Adopt a caller-supplied token when it is usable as a header value, so
            // applications can control the deduplication key.
            Some(existing) => {
                options = options.insert_extension(IdempotencyToken(existing.to_string()));
            }
            None => {
                let token = IdempotencyToken::new();
                let value =
                    http::HeaderValue::from_str(&token.0).expect("UUID v4 is a valid header value");
                headers.insert(
                    http::header::HeaderName::from_static(IDEMPOTENCY_TOKEN_HEADER),
                    value,
                );
                options = options.insert_extension(headers).insert_extension(token);
            }
        }
    }

    options
}

/// Helper function used by request models and handwritten methods to determine effective
/// idempotency and inject the `x-goog-gcs-idempotency-token` header extension when appropriate.
pub(crate) fn configure_idempotency(
    options: google_cloud_gax::options::RequestOptions,
    op: Operation,
) -> google_cloud_gax::options::RequestOptions {
    let (is_idempotent, is_mutating) = match op {
        Operation::Read => (true, false),
        Operation::Mutation { idempotent } => (idempotent, true),
    };
    let options =
        google_cloud_gax::options::internal::set_default_idempotency(options, is_idempotent);
    stamp_idempotency_token(options, is_mutating)
}

// Idempotency resolution for GCS requests follows the official GCS retry strategy:
// https://cloud.google.com/storage/docs/retry-strategy#idempotency-operations
//
// - Read and list operations are inherently idempotent and never attach a deduplication token.
// - Bucket creation, deletion, and retention lock operations are unconditionally idempotent.
// - Object mutations (writes, deletes, copies, restores) are idempotent only when protected by a
//   generation match precondition (or targeting a specific generation > 0).
// - Metadata updates are idempotent only when protected by a metageneration match precondition.
// - Negative preconditions (*_not_match) do NOT guarantee at-most-once semantics and are not idempotent.

impl crate::model::GetObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, Operation::Read)
    }
}

impl crate::model::ListObjectsRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, Operation::Read)
    }
}

impl crate::model::GetBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, Operation::Read)
    }
}

impl crate::model::ListBucketsRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, Operation::Read)
    }
}

impl crate::model::CreateBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, Operation::Mutation { idempotent: true })
    }
}

impl crate::model::DeleteBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, Operation::Mutation { idempotent: true })
    }
}

impl crate::model::LockBucketRetentionPolicyRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        configure_idempotency(options, Operation::Mutation { idempotent: true })
    }
}

impl crate::model::UpdateBucketRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_metageneration_match.is_some();
        configure_idempotency(
            options,
            Operation::Mutation {
                idempotent: is_idempotent,
            },
        )
    }
}

impl crate::model::ComposeObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_generation_match.is_some();
        configure_idempotency(
            options,
            Operation::Mutation {
                idempotent: is_idempotent,
            },
        )
    }
}

impl crate::model::DeleteObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.generation > 0 || self.if_generation_match.is_some();
        configure_idempotency(
            options,
            Operation::Mutation {
                idempotent: is_idempotent,
            },
        )
    }
}

impl crate::model::RestoreObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_generation_match.is_some();
        configure_idempotency(
            options,
            Operation::Mutation {
                idempotent: is_idempotent,
            },
        )
    }
}

impl crate::model::UpdateObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_metageneration_match.is_some();
        configure_idempotency(
            options,
            Operation::Mutation {
                idempotent: is_idempotent,
            },
        )
    }
}

impl crate::model::RewriteObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_generation_match.is_some();
        configure_idempotency(
            options,
            Operation::Mutation {
                idempotent: is_idempotent,
            },
        )
    }
}

impl crate::model::MoveObjectRequest {
    pub(crate) fn resolve_idempotency(
        &self,
        options: google_cloud_gax::options::RequestOptions,
    ) -> google_cloud_gax::options::RequestOptions {
        let is_idempotent = self.if_generation_match.is_some();
        configure_idempotency(
            options,
            Operation::Mutation {
                idempotent: is_idempotent,
            },
        )
    }
}

impl crate::model::WriteObjectSpec {
    pub(crate) fn is_idempotent(&self) -> bool {
        self.if_generation_match.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_cloud_gax::options::internal::RequestOptionsExt;
    use test_case::test_case;

    #[test]
    fn configure_idempotency_conditionally_safe_mutating() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = configure_idempotency(options, Operation::Mutation { idempotent: true });
        assert_eq!(resolved.idempotent(), Some(true));
        assert!(resolved.get_extension::<IdempotencyToken>().is_some());
        let headers = resolved
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));
    }

    #[test]
    fn configure_idempotency_not_safe_mutating() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = configure_idempotency(options, Operation::Mutation { idempotent: false });
        assert_eq!(resolved.idempotent(), Some(false));
        assert!(resolved.get_extension::<IdempotencyToken>().is_none());
        assert!(resolved.get_extension::<http::HeaderMap>().is_none());
    }

    #[test]
    fn configure_idempotency_reads_never_stamp_token() {
        let options = google_cloud_gax::options::RequestOptions::default();
        let resolved = configure_idempotency(options, Operation::Read);
        assert_eq!(resolved.idempotent(), Some(true));
        assert!(resolved.get_extension::<IdempotencyToken>().is_none());
        assert!(resolved.get_extension::<http::HeaderMap>().is_none());
    }

    #[test]
    fn per_request_override_takes_precedence() {
        let mut options = google_cloud_gax::options::RequestOptions::default();
        options.set_idempotency(true);
        let resolved = configure_idempotency(options, Operation::Mutation { idempotent: false });
        assert_eq!(resolved.idempotent(), Some(true));
        assert!(resolved.get_extension::<IdempotencyToken>().is_some());
        let headers = resolved
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));

        let mut options2 = google_cloud_gax::options::RequestOptions::default();
        options2.set_idempotency(false);
        let resolved2 = configure_idempotency(options2, Operation::Mutation { idempotent: true });
        assert_eq!(resolved2.idempotent(), Some(false));
        assert!(resolved2.get_extension::<IdempotencyToken>().is_none());
        assert!(resolved2.get_extension::<http::HeaderMap>().is_none());
    }

    #[test]
    fn stamp_idempotency_token_preserves_existing_token() {
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
    fn tokens_are_unique_across_requests() {
        let opts1 = google_cloud_gax::options::RequestOptions::default();
        let opts2 = google_cloud_gax::options::RequestOptions::default();
        let res1 = configure_idempotency(opts1, Operation::Mutation { idempotent: true });
        let res2 = configure_idempotency(opts2, Operation::Mutation { idempotent: true });
        let token1 = res1.get_extension::<IdempotencyToken>().unwrap();
        let token2 = res2.get_extension::<IdempotencyToken>().unwrap();
        assert_ne!(token1, token2, "each request must receive a distinct UUID");
    }

    #[test]
    fn custom_idempotency_token_header_synchronized() {
        let mut custom_headers = http::HeaderMap::new();
        custom_headers.insert(
            http::header::HeaderName::from_static(IDEMPOTENCY_TOKEN_HEADER),
            http::HeaderValue::from_static("custom-uuid-12345"),
        );
        let options =
            google_cloud_gax::options::RequestOptions::default().insert_extension(custom_headers);
        let resolved = configure_idempotency(options, Operation::Mutation { idempotent: true });
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
        let resolved = configure_idempotency(options, Operation::Mutation { idempotent: true });

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
        let resolved = configure_idempotency(options, Operation::Mutation { idempotent: true });

        let headers = resolved
            .get_extension::<http::HeaderMap>()
            .expect("header map exists");
        assert_eq!(
            headers.get("x-goog-custom").map(|v| v.as_bytes()),
            Some("keep-me".as_bytes())
        );
        assert!(headers.contains_key(IDEMPOTENCY_TOKEN_HEADER));
    }

    /// CI integrity guard: ensures all 14 RPCs in `gapic/transport.rs` use the generator hook
    /// `resolve_idempotency` and no unhooked `set_default_idempotency` calls remain.
    #[test]
    fn gapic_transport_idempotency_hook_integrity() {
        let transport_source = include_str!("generated/gapic/transport.rs");
        let resolve_count = transport_source.matches(".resolve_idempotency(").count();
        let set_default_count = transport_source.matches("set_default_idempotency(").count();
        assert_eq!(
            resolve_count, 14,
            "All 14 Storage RPCs in gapic/transport.rs must route through resolve_idempotency"
        );
        assert_eq!(
            set_default_count, 0,
            "gapic/transport.rs must not contain any unhooked set_default_idempotency calls"
        );
    }

    #[test_case(crate::model::GetObjectRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "get_object: reads are always idempotent")]
    #[test_case(crate::model::ListObjectsRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "list_objects: reads are always idempotent")]
    #[test_case(crate::model::GetBucketRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "get_bucket: reads are always idempotent")]
    #[test_case(crate::model::ListBucketsRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "list_buckets: reads are always idempotent")]
    #[test_case(crate::model::CreateBucketRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "create_bucket: unconditionally idempotent")]
    #[test_case(crate::model::DeleteBucketRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "delete_bucket: unconditionally idempotent")]
    #[test_case(crate::model::LockBucketRetentionPolicyRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "lock_bucket_retention: unconditionally idempotent")]
    #[test_case(crate::model::UpdateBucketRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "update_bucket: unconditioned is not idempotent")]
    #[test_case(crate::model::UpdateBucketRequest { if_metageneration_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "update_bucket: metageneration match")]
    #[test_case(crate::model::UpdateBucketRequest { if_metageneration_not_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "update_bucket: metageneration not_match is not idempotent")]
    #[test_case(crate::model::ComposeObjectRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "compose_object: unconditioned is not idempotent")]
    #[test_case(crate::model::ComposeObjectRequest { if_generation_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "compose_object: destination generation match")]
    #[test_case(crate::model::ComposeObjectRequest { if_metageneration_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "compose_object: metageneration match is not idempotent")]
    #[test_case(crate::model::DeleteObjectRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "delete_object: unconditioned is not idempotent")]
    #[test_case(crate::model::DeleteObjectRequest { generation: 12345, ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "delete_object: specific generation > 0")]
    #[test_case(crate::model::DeleteObjectRequest { generation: 0, ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "delete_object: generation 0 is unconditioned")]
    #[test_case(crate::model::DeleteObjectRequest { if_generation_match: Some(12345), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "delete_object: generation match")]
    #[test_case(crate::model::DeleteObjectRequest { if_generation_not_match: Some(12345), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "delete_object: generation not_match is not idempotent")]
    #[test_case(crate::model::DeleteObjectRequest { if_metageneration_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "delete_object: metageneration match is not idempotent for object delete")]
    #[test_case(crate::model::DeleteObjectRequest { if_metageneration_not_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "delete_object: metageneration not_match is not idempotent")]
    #[test_case(crate::model::RestoreObjectRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "restore_object: unconditioned is not idempotent")]
    #[test_case(crate::model::RestoreObjectRequest { if_generation_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "restore_object: generation match")]
    #[test_case(crate::model::RestoreObjectRequest { if_generation_not_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "restore_object: generation not_match is not idempotent")]
    #[test_case(crate::model::UpdateObjectRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "update_object: unconditioned is not idempotent")]
    #[test_case(crate::model::UpdateObjectRequest { if_metageneration_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "update_object: metageneration match")]
    #[test_case(crate::model::UpdateObjectRequest { if_metageneration_not_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "update_object: metageneration not_match is not idempotent")]
    #[test_case(crate::model::UpdateObjectRequest { if_generation_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "update_object: generation match alone does not make metadata update idempotent")]
    #[test_case(crate::model::RewriteObjectRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "rewrite_object: unconditioned is not idempotent")]
    #[test_case(crate::model::RewriteObjectRequest { if_generation_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "rewrite_object: destination generation match")]
    #[test_case(crate::model::RewriteObjectRequest { if_generation_not_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "rewrite_object: generation not_match is not idempotent")]
    #[test_case(crate::model::RewriteObjectRequest { if_source_generation_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "rewrite_object: source generation match alone does not protect destination")]
    #[test_case(crate::model::MoveObjectRequest::default().resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "move_object: unconditioned is not idempotent")]
    #[test_case(crate::model::MoveObjectRequest { if_generation_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), true; "move_object: destination generation match")]
    #[test_case(crate::model::MoveObjectRequest { if_generation_not_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "move_object: generation not_match is not idempotent")]
    #[test_case(crate::model::MoveObjectRequest { if_source_generation_match: Some(1), ..Default::default() }.resolve_idempotency(google_cloud_gax::options::RequestOptions::default()).idempotent() == Some(true), false; "move_object: source generation match alone does not protect destination")]
    #[test_case(crate::model::WriteObjectSpec::default().is_idempotent(), false; "write_object_spec: unconditioned is not idempotent")]
    #[test_case(crate::model::WriteObjectSpec { if_generation_match: Some(0), ..Default::default() }.is_idempotent(), true; "write_object_spec: generation match")]
    #[test_case(crate::model::WriteObjectSpec { if_generation_not_match: Some(0), ..Default::default() }.is_idempotent(), false; "write_object_spec: generation not_match is not idempotent")]
    fn request_idempotency_evaluations(actual: bool, expected: bool) {
        assert_eq!(actual, expected);
    }
}
