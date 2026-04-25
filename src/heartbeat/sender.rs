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
use std::time::Instant;

// Package
use serde::Serialize;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

// Local
use crate::containerisation::traits::HyperionHeartbeatMessage;
use crate::heartbeat::handler::HeartbeatMissedHandler;
use crate::messages::heartbeat::HeartbeatResponse;
use crate::utilities::time::{current_epoch_ms, fmt_ms};


pub struct HeartbeatSender<T> {
    interval_ms: u64,
    response_timeout_ms: u64,
    container_name: String,
    target_senders: HashMap<String, mpsc::Sender<T>>,
    response_rx: mpsc::Receiver<HeartbeatResponse>,
    next_id: u64,
    pending: HashMap<u64, (String, Instant)>,
    missed_handler: Option<Box<dyn HeartbeatMissedHandler>>,
}

impl<T> HeartbeatSender<T>
where
    T: HyperionHeartbeatMessage + Send + Clone + Serialize + 'static,
{
    /// Returns the sender task and a tx handle the container uses to forward responses into it.
    pub fn new(
        interval_ms: u64,
        response_timeout_ms: u64,
        container_name: String,
        all_senders: HashMap<String, mpsc::Sender<T>>,
        targets: &[String],
        missed_handler: Option<Box<dyn HeartbeatMissedHandler>>,
    ) -> (Self, mpsc::Sender<HeartbeatResponse>) {
        let target_senders: HashMap<String, mpsc::Sender<T>> = targets
            .iter()
            .filter_map(|name| {
                if let Some(s) = all_senders.get(name) {
                    Some((name.clone(), s.clone()))
                } else {
                    log::warn!("HeartbeatSender: target '{}' not found in client senders — skipping", name);
                    None
                }
            })
            .collect();

        let (response_tx, response_rx) = mpsc::channel(32);
        (
            Self {
                interval_ms,
                response_timeout_ms,
                container_name,
                target_senders,
                response_rx,
                next_id: 0,
                pending: HashMap::new(),
                missed_handler,
            },
            response_tx
        )
    }

    pub async fn run(mut self) {
        let mut ticker = interval(Duration::from_millis(self.interval_ms));
        ticker.tick().await; // consume the immediate first tick
        log::info!("HeartbeatSender running — targets: {:?}", self.target_senders.keys().collect::<Vec<_>>());

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let now_ms = current_epoch_ms();
                    for (name, sender) in &self.target_senders {
                        self.next_id += 1;
                        let id = self.next_id;
                        if let Some(msg) = T::make_heartbeat_request(id, self.container_name.clone(), now_ms) {
                            if sender.try_send(msg).is_err() {
                                log::warn!("HeartbeatSender: failed to queue request to '{}'", name);
                            }
                            self.pending.insert(id, (name.clone(), Instant::now()));
                        }
                    }
                    // Prune requests that have exceeded the response timeout
                    let timeout = Duration::from_millis(self.response_timeout_ms);
                    let missed_handler = &self.missed_handler;
                    self.pending.retain(|id, (target, sent_at)| {
                        if sent_at.elapsed() > timeout {
                            log::warn!("HeartbeatSender: no response from '{}' for request #{}", target, id);
                            if let Some(handler) = missed_handler {
                                handler.on_missed(target);
                            }
                            false
                        } else {
                            true
                        }
                    });
                }
                Some(response) = self.response_rx.recv() => {
                    if let Some((target, sent_at)) = self.pending.remove(&response.request_id) {
                        let latency_ms = sent_at.elapsed().as_millis();
                        if response.component_alive {
                            log::debug!(
                                "Heartbeat report ✓ {} | {} latency | activity {} ago | state {}",
                                target.to_ascii_uppercase(), fmt_ms(latency_ms), fmt_ms(response.ms_since_last_activity as u128), response.container_state_val
                            );
                        } else {
                            log::warn!(
                                "Heartbeat report ✓ {} | {} latency | COMPONENT NOT ALIVE | activity {} ago",
                                target.to_ascii_uppercase(), fmt_ms(latency_ms), fmt_ms(response.ms_since_last_activity as u128)
                            );
                        }
                    }
                }
            }
        }
    }
}
