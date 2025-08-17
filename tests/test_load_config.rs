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
use std::path::PathBuf;

// Package
use hyperion_framework::utilities::load_config::load_config;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct MiniCfg {
    name: String,
    version: String,
}

#[test]
fn loads_valid_xml_into_struct() {
    let tmp_dir = tempfile::tempdir().expect("tmpdir");
    let tmp_path = tmp_dir.path().to_path_buf();
    let file_path: PathBuf = tmp_path.join("mini.xml");

    let xml = r#"
        <MiniCfg>
            <name>Hyperion</name>
            <version>1.2.3</version>
        </MiniCfg>
        "#;
    std::fs::write(&file_path, xml).expect("write xml");

    let cfg = load_config::<MiniCfg>(&file_path).expect("should load");
    assert_eq!(&cfg.name, "Hyperion");
    assert_eq!(&cfg.version, "1.2.3");
}

#[test]
fn returns_error_on_missing_file() {
    let bogus = PathBuf::from("/this/path/should/not/exist/never.xml");
    let res = load_config::<MiniCfg>(&bogus);
    assert!(res.is_err());
}

#[test]
fn returns_error_on_malformed_xml() {
    let tmp_dir = tempfile::tempdir().expect("tmpdir");
    let tmp_path = tmp_dir.path().to_path_buf();
    let file_path: PathBuf = tmp_path.join("bad.xml");

    // Missing closing tag
    let xml = r#"
        <MiniCfg>
            <name>x</name>
            <version>y"#;
    std::fs::write(&file_path, xml).expect("write xml");

    let res = load_config::<MiniCfg>(&file_path);
    assert!(res.is_err());
}
