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

/// Represents a Google Cloud Storage service account (service agent).
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
#[non_exhaustive]
pub struct ServiceAccount {
    /// The email address of the service account.
    pub email_address: String,
    /// The kind of item this is. For service accounts, this is always `storage#serviceAccount`.
    pub kind: String,
}

impl ServiceAccount {
    /// Creates a new `ServiceAccount` with the given email address.
    pub fn new(email_address: impl Into<String>) -> Self {
        Self {
            email_address: email_address.into(),
            kind: "storage#serviceAccount".to_string(),
        }
    }
}
