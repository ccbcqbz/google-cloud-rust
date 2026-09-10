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

//! A shared `httptest` responder that records the idempotency token sent on
//! each attempt.
//!
//! The upload tests need to assert that GAX replays the identical
//! `x-goog-gcs-idempotency-token` after a transient failure. That requires
//! inspecting the request headers, which the stock `httptest` responders cannot
//! do, so all the upload test modules share this responder.

use std::sync::{Arc, Mutex, atomic::AtomicUsize};

/// The tokens observed by a [TokenCapture], in request order.
///
/// An entry is `None` when the request carried no idempotency token.
pub(crate) type CapturedTokens = Arc<Mutex<Vec<Option<String>>>>;

/// Fails the first request with `503` and succeeds afterwards, recording the
/// idempotency token seen on every attempt.
pub(crate) struct TokenCapture {
    tokens: CapturedTokens,
    call_count: AtomicUsize,
    success_headers: Vec<(&'static str, String)>,
    success_body: bytes::Bytes,
}

impl TokenCapture {
    /// Responds like the "create resumable upload session" endpoint, returning
    /// `session_url` in the `location` header once the transient failure is
    /// past.
    pub(crate) fn resumable_session(tokens: CapturedTokens, session_url: String) -> Self {
        Self {
            tokens,
            call_count: AtomicUsize::new(0),
            success_headers: vec![("location", session_url)],
            success_body: bytes::Bytes::new(),
        }
    }

    /// Responds with a JSON object payload once the transient failure is past.
    pub(crate) fn json_body(tokens: CapturedTokens, body: bytes::Bytes) -> Self {
        Self {
            tokens,
            call_count: AtomicUsize::new(0),
            success_headers: vec![("content-type", "application/json".to_string())],
            success_body: body,
        }
    }
}

impl httptest::responders::Responder for TokenCapture {
    fn respond<'a>(
        &mut self,
        req: &'a http::Request<bytes::Bytes>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = http::Response<bytes::Bytes>> + std::marker::Send + 'a>,
    > {
        let token = req
            .headers()
            .get(crate::idempotency::IDEMPOTENCY_TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        self.tokens.lock().unwrap().push(token);
        let count = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let res = if count == 0 {
            http::Response::builder()
                .status(503)
                .body(bytes::Bytes::from("try-again"))
                .unwrap()
        } else {
            let mut builder = http::Response::builder().status(200);
            for (name, value) in &self.success_headers {
                builder = builder.header(*name, value);
            }
            builder.body(self.success_body.clone()).unwrap()
        };
        Box::pin(async move { res })
    }
}
