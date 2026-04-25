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

// Standard
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// Package
use serde::Serialize;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

// Local
use crate::containerisation::traits::HyperionHeartbeatMessage;
use crate::heartbeat::handler::HeartbeatTimeoutHandler;
use crate::messages::heartbeat::HeartbeatRequest;
use crate::utilities::time::current_epoch_ms;

pub struct HeartbeatReceiver<T> {
    timeout_ms: u64,
    last_received_ms: Arc<AtomicU64>,
    request_rx: mpsc::Receiver<HeartbeatRequest>,
    all_senders: HashMap<String, mpsc::Sender<T>>, // Copy of client map from ClientBroker
    handler: Box<dyn HeartbeatTimeoutHandler>,
}

impl<T> HeartbeatReceiver<T>
where
    T: HyperionHeartbeatMessage + Send + Clone + Serialize + 'static,
{
    /// Returns the receiver task and a tx handle the container uses to forward enriched requests
    /// into it.
    pub fn new(
        timeout_ms: u64,
        all_senders: HashMap<String, mpsc::Sender<T>>,
        handler: Box<dyn HeartbeatTimeoutHandler>,
    ) -> (Self, mpsc::Sender<HeartbeatRequest>) {
        let (request_tx, request_rx) = mpsc::channel(32);
        (
            Self {
                timeout_ms,
                last_received_ms: Arc::new(AtomicU64::new(current_epoch_ms())),
                request_rx,
                all_senders,
                handler,
            },
            request_tx,
        )
    }

    pub async fn run(mut self) {
        // Check three times per timeout window so we catch a miss promptly.
        let check_interval_ms = (self.timeout_ms / 3).max(1000);
        let mut watchdog = interval(Duration::from_millis(check_interval_ms));
        log::info!("HeartbeatReceiver running — timeout: {}ms", self.timeout_ms);

        loop {
            tokio::select! {
                Some(request) = self.request_rx.recv() => {
                    self.last_received_ms.store(current_epoch_ms(), Ordering::SeqCst);

                    if let Some(sender) = self.all_senders.get(&request.sender_name) {
                        if let Some(msg) = T::make_heartbeat_response(
                            request.request_id,
                            current_epoch_ms(),
                            request.component_alive,
                            request.ms_since_last_activity,
                            request.container_state_val,
                        ) {
                            if sender.try_send(msg).is_err() {
                                log::warn!("HeartbeatReceiver: failed to send response to '{}'", request.sender_name);
                            }
                        }
                    } else {
                        log::warn!("HeartbeatReceiver: no sender found for '{}' — cannot respond", request.sender_name);
                    }
                }
                _ = watchdog.tick() => {
                    let elapsed_ms = current_epoch_ms()
                        .saturating_sub(self.last_received_ms.load(Ordering::SeqCst));
                    if elapsed_ms > self.timeout_ms {
                        log::warn!(
                            "HeartbeatReceiver: {}ms since last request (timeout: {}ms) — calling handler",
                            elapsed_ms, self.timeout_ms
                        );
                        self.handler.on_timeout();
                    }
                }
            }
        }
    }
}
