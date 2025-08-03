// -------------------------------------------------------------------------------------------------
// Hyperion Framework
// https://github.com/robert-hannah/hyperion-framework
//
// A lightweight component-based TCP framework for building service-oriented Rust applications with
// CLI control, async messaging, and lifecycle management.
//
// Copyright 2025 Robert Hannah
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
// -------------------------------------------------------------------------------------------------

// Package
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

pub async fn add_to_tx_with_retry<T>(
    tx: &mpsc::Sender<T>,
    message: &T,
    from_location: &str,
    to_location: &str,
) where
    T: Clone + Send + Sync,
{
    let mut attempts = 0;
    let max_attempts = 5;

    loop {
        match tx.try_send(message.clone()) {
            Ok(_) => {
                // log::debug!("Message forwarded to main successfully.");
                break;
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                attempts += 1;
                if attempts >= max_attempts {
                    log::error!(
                        "Failed to add message to TX channel from {from_location} to {to_location} after {max_attempts} attempts"
                    );
                    break;
                }
                // Exponential backoff before retrying
                sleep(Duration::from_millis(100 * 2u64.pow(attempts))).await;
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                log::error!(
                    "TX channel is closed. Cannot send message from {from_location} to {to_location}"
                );
                break;
            }
        }
    }
}
