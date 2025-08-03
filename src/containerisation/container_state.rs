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
use serde::{Deserialize, Serialize};

#[repr(usize)]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ContainerState {
    Running = 0,            // Container is running
    MissingConnections = 1, // Container has missing connections which can be restarted
    DeadComponent = 2,      // Component is dead but can be restarted
    ShuttingDown = 3,       // Container is closing
    Closed = 4,             // Container is closed and cannot be restarted
}

impl From<usize> for ContainerState {
    fn from(value: usize) -> Self {
        match value {
            0 => ContainerState::Running,
            1 => ContainerState::MissingConnections,
            2 => ContainerState::DeadComponent,
            3 => ContainerState::ShuttingDown,
            4 => ContainerState::Closed,
            _ => panic!("Invalid component state value"),
        }
    }
}
