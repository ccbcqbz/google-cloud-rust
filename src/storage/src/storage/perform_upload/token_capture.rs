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

//! A shared `httptest` responder that records the idempotency token sent on
//! each attempt.
//!
//! The upload tests need to assert that GAX replays the identical
//! `x-goog-gcs-idempotency-token` after a transient failure. That requires
//! inspecting the request headers, which the stock `httptest` responders cannot
//! do, so all the upload test modules share this responder.

use std::sync::{Arc, Mutex};

/// The tokens observed by a [TokenCapture], in request order.
///
/// An entry is `None` when the request carried no idempotency token.
pub(crate) type CapturedTokens = Arc<Mutex<Vec<Option<String>>>>;

enum Success {
    Session(String),
    Json(bytes::Bytes),
}

/// Fails the first request with `503` and succeeds afterwards, recording the
/// idempotency token seen on every attempt.
pub(crate) struct TokenCapture {
    tokens: CapturedTokens,
    call_count: usize,
    success: Success,
}

impl TokenCapture {
    /// Responds like the "create resumable upload session" endpoint, returning
    /// `session_url` in the `location` header once the transient failure is
    /// past.
    pub(crate) fn resumable_session(tokens: CapturedTokens, session_url: String) -> Self {
        Self {
            tokens,
            call_count: 0,
            success: Success::Session(session_url),
        }
    }

    /// Responds with a JSON object payload once the transient failure is past.
    pub(crate) fn json_body(tokens: CapturedTokens, body: bytes::Bytes) -> Self {
        Self {
            tokens,
            call_count: 0,
            success: Success::Json(body),
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
        let count = self.call_count;
        self.call_count += 1;
        let res = if count == 0 {
            http::Response::builder()
                .status(503)
                .body(bytes::Bytes::from("try-again"))
                .unwrap()
        } else {
            match &self.success {
                Success::Session(url) => http::Response::builder()
                    .status(200)
                    .header("location", url)
                    .body(bytes::Bytes::new())
                    .unwrap(),
                Success::Json(body) => http::Response::builder()
                    .status(200)
                    .header("content-type", "application/json")
                    .body(body.clone())
                    .unwrap(),
            }
        };
        Box::pin(async move { res })
    }
}

/// Shared helper asserting token reuse across retries when starting a resumable
/// upload, and verifying that subsequent data `PUT` requests carry no token.
#[cfg(test)]
pub(crate) async fn assert_resumable_retry_token_reuse<F, Fut>(run_upload: F) -> anyhow::Result<()>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    use httptest::{Expectation, Server, matchers::*, responders::*};

    let server = Server::run();
    let session = server.url("/upload/session/test-only-001");
    let path = session.path().to_string();

    let captured_tokens = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let responder = TokenCapture::resumable_session(captured_tokens.clone(), session.to_string());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("POST", "/upload/storage/v1/b/test-bucket/o"),
            request::query(url_decoded(contains(("name", "test-object")))),
            request::query(url_decoded(contains(("uploadType", "resumable")))),
        ])
        .times(2)
        .respond_with(responder),
    );
    server.expect(
        Expectation::matching(all_of![
            request::method_path("PUT", path.clone()),
            request::headers(contains(("content-range", "bytes */0"))),
            not(request::headers(contains(key(
                crate::idempotency::IDEMPOTENCY_TOKEN_HEADER
            )))),
        ])
        .respond_with(
            status_code(200)
                .append_header("content-type", "application/json")
                .body(
                    serde_json::json!({
                        "kind": "storage#object",
                        "id": "test-bucket/test-object/1",
                        "name": "test-object",
                        "bucket": "test-bucket",
                        "generation": "1",
                        "metageneration": "1",
                        "size": "0",
                    })
                    .to_string(),
                ),
        ),
    );

    run_upload(format!("http://{}", server.addr())).await?;

    let tokens = captured_tokens.lock().unwrap().clone();
    assert_eq!(tokens.len(), 2, "must attempt 2 POST requests");
    assert!(
        tokens[0].is_some(),
        "first attempt must have idempotency token"
    );
    assert_eq!(
        tokens[0], tokens[1],
        "token must be identical across retries"
    );

    Ok(())
}
