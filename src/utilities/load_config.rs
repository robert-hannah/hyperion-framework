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

// Standard
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc as StdArc;

// Package
use serde::de::DeserializeOwned;
use serde_xml_rs::from_str;

pub fn load_config<T: DeserializeOwned>(
    absolute_file_path: &PathBuf,
) -> Result<StdArc<T>, Box<dyn std::error::Error>> {
    let mut file = File::open(absolute_file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: T = from_str(&contents)?;
    Ok(StdArc::new(config))
}
