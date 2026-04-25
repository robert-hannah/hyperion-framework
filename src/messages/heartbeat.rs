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

/// Enriched form of a HeartbeatRequest, assembled by the container before handing off to the
/// receiver task. Includes component health data that only the container can observe.
#[derive(Debug, Clone)]
pub struct HeartbeatRequest {
    pub request_id: u64,
    pub sender_name: String,
    pub timestamp_ms: u64,
    pub component_alive: bool,
    pub ms_since_last_activity: u64,
    pub container_state_val: usize
}

/// Parsed form of a HeartbeatResponse, forwarded by the container to the sender task.
#[derive(Debug, Clone)]
pub struct HeartbeatResponse {
    pub request_id: u64,
    pub responder_name: String,
    pub timestamp_ms: u64,
    pub component_alive: bool,
    pub ms_since_last_activity: u64,
    pub container_state_val: usize
}
