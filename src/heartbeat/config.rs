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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum HeartbeatMode {
    Sender,
    Receiver,
    Disabled
}

#[derive(Debug, Clone)]
pub struct HeartbeatSenderConfig {
    pub interval_ms: u64,
    pub response_timeout_ms: u64,
    pub targets: Vec<String>
}

#[derive(Debug, Clone)]
pub struct HeartbeatReceiverConfig {
    /// How long to wait without receiving a request before triggering the timeout handler.
    pub timeout_ms: u64
}

#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    pub mode: HeartbeatMode,
    pub sender: Option<HeartbeatSenderConfig>,
    pub receiver: Option<HeartbeatReceiverConfig>
}

impl HeartbeatConfig {
    pub fn sender(interval_ms: u64, response_timeout_ms: u64, targets: Vec<String>) -> Self {
        Self {
            mode: HeartbeatMode::Sender,
            sender: Some(HeartbeatSenderConfig {
                interval_ms,
                response_timeout_ms,
                targets,
            }),
            receiver: None
        }
    }

    pub fn receiver(timeout_ms: u64) -> Self {
        Self {
            mode: HeartbeatMode::Receiver,
            sender: None,
            receiver: Some(HeartbeatReceiverConfig { timeout_ms }),
        }
    }

    pub fn disabled() -> Self {
        Self {
            mode: HeartbeatMode::Disabled,
            sender: None,
            receiver: None,
        }
    }
}

/// Deserializable form of heartbeat configuration as it appears in a component's
/// `configuration.xml` under the `<container>` section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatXml {
    pub mode: String,
    #[serde(default)]
    pub interval_ms: Option<u64>,
    #[serde(default)]
    pub response_timeout_ms: Option<u64>,
    #[serde(default)]
    pub targets: Option<HeartbeatTargets>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatTargets {
    #[serde(rename = "target")]
    pub targets: Vec<String>,
}

impl HeartbeatXml {
    pub fn to_config(&self) -> Option<HeartbeatConfig> {
        match self.mode.as_str() {
            "sender" => {
                let interval_ms = self.interval_ms
                    .unwrap_or_else(|| panic!("Heartbeat mode is 'sender' but interval_ms is missing"));
                let response_timeout_ms = self.response_timeout_ms
                    .unwrap_or_else(|| panic!("Heartbeat mode is 'sender' but response_timeout_ms is missing"));
                let targets = self.targets.as_ref()
                    .map(|t| t.targets.clone())
                    .unwrap_or_default();
                Some(HeartbeatConfig::sender(interval_ms, response_timeout_ms, targets))
            }
            "receiver" => {
                let timeout_ms = self.timeout_ms
                    .unwrap_or_else(|| panic!("Heartbeat mode is 'receiver' but timeout_ms is missing"));
                Some(HeartbeatConfig::receiver(timeout_ms))
            }
            "disabled" => None,
            unknown => panic!("Unknown heartbeat mode: '{unknown}'. Expected 'sender', 'receiver', or 'disabled'"),
        }
    }
}
