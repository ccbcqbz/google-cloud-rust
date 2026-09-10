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

use google_cloud_auth::credentials::anonymous::Builder as Anonymous;
use google_cloud_storage::client::StorageControl;
use storage_grpc_mock::{MockStorage, start};

const BIND_ADDRESS: &str = "127.0.0.1:0";
const BUCKET_NAME: &str = "projects/_/buckets/test-bucket";
const OBJECT_NAME: &str = "test-object";
const IDEMPOTENCY_TOKEN_HEADER: &str = "x-goog-gcs-idempotency-token";

#[tokio::test]
async fn delete_object_with_generation_sends_idempotency_token() -> anyhow::Result<()> {
    let (observed_tx, observed_rx) = tokio::sync::oneshot::channel::<Option<String>>();

    let mut mock = MockStorage::new();
    mock.expect_delete_object().return_once(move |request| {
        let token = request
            .metadata()
            .get(IDEMPOTENCY_TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let _ = observed_tx.send(token);
        Ok(gaxi::grpc::tonic::Response::new(()))
    });

    let (endpoint, _server) = start(BIND_ADDRESS, mock).await?;
    let client = StorageControl::builder()
        .with_endpoint(endpoint)
        .with_credentials(Anonymous::default().build())
        .build()
        .await?;

    client
        .delete_object()
        .set_bucket(BUCKET_NAME)
        .set_object(OBJECT_NAME)
        .set_generation(12345)
        .send()
        .await?;

    let token = observed_rx.await?;
    assert!(
        token.is_some(),
        "DeleteObject with generation > 0 must attach x-goog-gcs-idempotency-token"
    );
    // Token must be a valid UUID v4
    let uuid_str = token.unwrap();
    assert!(
        uuid::Uuid::parse_str(&uuid_str).is_ok(),
        "token must be valid UUID: {uuid_str}"
    );

    Ok(())
}

#[tokio::test]
async fn delete_object_with_precondition_sends_idempotency_token() -> anyhow::Result<()> {
    let (observed_tx, observed_rx) = tokio::sync::oneshot::channel::<Option<String>>();

    let mut mock = MockStorage::new();
    mock.expect_delete_object().return_once(move |request| {
        let token = request
            .metadata()
            .get(IDEMPOTENCY_TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let _ = observed_tx.send(token);
        Ok(gaxi::grpc::tonic::Response::new(()))
    });

    let (endpoint, _server) = start(BIND_ADDRESS, mock).await?;
    let client = StorageControl::builder()
        .with_endpoint(endpoint)
        .with_credentials(Anonymous::default().build())
        .build()
        .await?;

    client
        .delete_object()
        .set_bucket(BUCKET_NAME)
        .set_object(OBJECT_NAME)
        .set_if_generation_match(54321)
        .send()
        .await?;

    let token = observed_rx.await?;
    assert!(
        token.is_some(),
        "DeleteObject with if_generation_match must attach x-goog-gcs-idempotency-token"
    );

    Ok(())
}

#[tokio::test]
async fn delete_object_unconditioned_omits_idempotency_token() -> anyhow::Result<()> {
    let (observed_tx, observed_rx) = tokio::sync::oneshot::channel::<Option<String>>();

    let mut mock = MockStorage::new();
    mock.expect_delete_object().return_once(move |request| {
        let token = request
            .metadata()
            .get(IDEMPOTENCY_TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let _ = observed_tx.send(token);
        Ok(gaxi::grpc::tonic::Response::new(()))
    });

    let (endpoint, _server) = start(BIND_ADDRESS, mock).await?;
    let client = StorageControl::builder()
        .with_endpoint(endpoint)
        .with_credentials(Anonymous::default().build())
        .build()
        .await?;

    // DeleteObject WITHOUT preconditions (generation: 0, no match preconditions)
    client
        .delete_object()
        .set_bucket(BUCKET_NAME)
        .set_object(OBJECT_NAME)
        .send()
        .await?;

    let token = observed_rx.await?;
    assert_eq!(
        token, None,
        "DeleteObject without preconditions must NOT attach x-goog-gcs-idempotency-token"
    );

    Ok(())
}

#[tokio::test]
async fn get_object_read_omits_idempotency_token() -> anyhow::Result<()> {
    let (observed_tx, observed_rx) = tokio::sync::oneshot::channel::<Option<String>>();

    let mut mock = MockStorage::new();
    mock.expect_get_object().return_once(move |request| {
        let token = request
            .metadata()
            .get(IDEMPOTENCY_TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let _ = observed_tx.send(token);
        Ok(gaxi::grpc::tonic::Response::new(
            storage_grpc_mock::google::storage::v2::Object {
                name: OBJECT_NAME.to_string(),
                bucket: BUCKET_NAME.to_string(),
                generation: 1,
                ..Default::default()
            },
        ))
    });

    let (endpoint, _server) = start(BIND_ADDRESS, mock).await?;
    let client = StorageControl::builder()
        .with_endpoint(endpoint)
        .with_credentials(Anonymous::default().build())
        .build()
        .await?;

    client
        .get_object()
        .set_bucket(BUCKET_NAME)
        .set_object(OBJECT_NAME)
        .send()
        .await?;

    let token = observed_rx.await?;
    assert_eq!(
        token, None,
        "GetObject (read operation) must NOT attach x-goog-gcs-idempotency-token"
    );

    Ok(())
}
