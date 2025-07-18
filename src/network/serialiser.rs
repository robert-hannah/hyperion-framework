// -------------------------------------------------------------------------------------------------
// Hyperion Framework
// https://github.com/Bazzz-1/hyperion-framework
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
use serde::{Serialize, de::DeserializeOwned};
use serde_json;

// Serialises any message into a byte vector (JSON format).
pub fn serialise_message<T: Serialize>(msg: &T) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(msg)
}

// Deserialises a byte slice into any message type.
pub fn deserialise_message<T: DeserializeOwned>(data: &[u8]) -> Result<T, serde_json::Error> {
    serde_json::from_slice(data)
}

// Example usage:
// Serialise
// let bytes = serialise_message(&msg).expect("Serialisation failed");
// println!("Serialized: {:?}", bytes);

// Deserialise
// let message: T = serialiser::deserialise_message(&buf[..n]).expect("Message deserialisation failed");
// println!("Deserialized: {:?}", message);
