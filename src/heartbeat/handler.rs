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

// Standard =
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Package
use tokio::sync::Notify;

// Local
use crate::containerisation::container_state::ContainerState;

/// Called by the receiver watchdog when no heartbeat request has arrived within the timeout window.
pub trait HeartbeatTimeoutHandler: Send + Sync + 'static {
    fn on_timeout(&self);
}

/// Called by the sender on every heartbeat tick where a target has not responded within
/// `response_timeout_ms`. Fires repeatedly while the target remains silent.
pub trait HeartbeatMissedHandler: Send + Sync + 'static {
    fn on_missed(&self, target: &str);
}

/// Wraps a `FnMut(&str)` closure as a `HeartbeatMissedHandler`.
pub struct FnMissedHandler(Box<dyn Fn(&str) + Send + Sync>);

impl FnMissedHandler {
    pub fn new(f: impl Fn(&str) + Send + Sync + 'static) -> Self {
        Self(Box::new(f))
    }
}

impl HeartbeatMissedHandler for FnMissedHandler {
    fn on_missed(&self, target: &str) {
        (self.0)(target);
    }
}

/// Wraps a closure as a `HeartbeatTimeoutHandler`, allowing per-component custom behaviour to be
/// defined inline in `main.rs` without needing a dedicated struct.
pub struct FnHandler(Box<dyn Fn() + Send + Sync>);

impl FnHandler {
    pub fn new(f: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Box::new(f))
    }
}

impl HeartbeatTimeoutHandler for FnHandler {
    fn on_timeout(&self) {
        (self.0)();
    }
}

/// Convenience handler: initiates a graceful container shutdown.
pub struct ShutdownOnTimeout {
    container_state: Arc<AtomicUsize>,
    container_state_notify: Arc<Notify>,
}

impl ShutdownOnTimeout {
    pub fn new(container_state: Arc<AtomicUsize>, container_state_notify: Arc<Notify>) -> Self {
        Self {
            container_state,
            container_state_notify,
        }
    }
}

impl HeartbeatTimeoutHandler for ShutdownOnTimeout {
    fn on_timeout(&self) {
        log::warn!(
            "HeartbeatReceiver: timeout — no request received within window. Initiating shutdown."
        );
        self.container_state
            .store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
        self.container_state_notify.notify_waiters();
    }
}
