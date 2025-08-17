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
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc as StdArc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Package
use hyperion_framework::containerisation::container_state::ContainerState;
use hyperion_framework::containerisation::hyperion_container::HyperionContainer;
use hyperion_framework::containerisation::hyperion_container_factory;
use hyperion_framework::containerisation::traits::{
    ContainerIdentidy, HyperionContainerDirectiveMessage, Initialisable, LogLevel, Run,
};
use hyperion_framework::messages::client_broker_message::ClientBrokerMessage;
use hyperion_framework::messages::container_directive::ContainerDirective;
use serde::Deserialize;
use tokio::sync::{Notify, mpsc};
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
enum TestMessage {
    Framework(ContainerDirective),
    Noop,
}

impl HyperionContainerDirectiveMessage for TestMessage {
    fn get_container_directive_message(&self) -> Option<&ContainerDirective> {
        if let TestMessage::Framework(d) = self {
            Some(d)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
struct DummyComponent;

#[async_trait::async_trait]
impl Run for DummyComponent {
    type Message = TestMessage;
    async fn run(
        self,
        mut comp_in_rx: mpsc::Receiver<Self::Message>,
        _comp_out_tx: mpsc::Sender<ClientBrokerMessage<Self::Message>>,
    ) {
        // Exit on Shutdown; ignore other messages
        while let Some(msg) = comp_in_rx.recv().await {
            if let TestMessage::Framework(ContainerDirective::Shutdown) = msg {
                break;
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct DummyLogging {
    level: String,
}
#[derive(Debug, Deserialize)]
struct DummyContainer {
    name: String,
}

#[derive(Debug, Deserialize)]
struct DummyConfig {
    logging: DummyLogging,
    container: DummyContainer,
}

impl LogLevel for DummyConfig {
    fn log_level(&self) -> &str {
        &self.logging.level
    }
}
impl ContainerIdentidy for DummyConfig {
    fn container_identity(&self) -> HashMap<String, String> {
        let mut id = HashMap::new();
        id.insert("name".into(), self.container.name.clone());
        id
    }
}

impl Initialisable for DummyComponent {
    type ConfigType = DummyConfig;
    fn initialise(
        _container_state: StdArc<AtomicUsize>,
        _container_state_notify: StdArc<Notify>,
        _config: StdArc<Self::ConfigType>,
    ) -> Self {
        DummyComponent
    }
}

fn write_temp_file(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    let mut f = fs::File::create(&path).expect("Create temp file");
    f.write_all(contents.as_bytes()).expect("Write temp file");
    path
}

#[tokio::test]
async fn factory_create_builds_and_runs_container() {
    // Create a temporary directory for configs
    let tmp_dir = tempfile::tempdir().expect("tmpdir");
    let tmp_path = tmp_dir.path().to_path_buf();

    let config_xml = r#"
        <DummyConfig>
            <logging>
                <level>Trace</level>
            </logging>
            <container>
                <name>TestContainer</name>
            </container>
        </DummyConfig>
        "#;

    // No client connections, server binds to ephemeral port 0
    let network_xml = r#"
        <NetworkTopology>
            <container_name>TestContainer</container_name>
            <server_address>127.0.0.1:0</server_address>
            <client_connections>
                <connection>
                    <name>c1</name>
                    <address>127.0.0.1:0</address>
                </connection>
            </client_connections>
        </NetworkTopology>
        "#;

    let config_path = write_temp_file(&tmp_path, "config.xml", config_xml);
    let topology_path = write_temp_file(&tmp_path, "network.xml", network_xml);

    let container_state: StdArc<AtomicUsize> =
        StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let container_state_notify: StdArc<Notify> = StdArc::new(Notify::new());

    let (main_tx, main_rx) = mpsc::channel::<TestMessage>(32);

    let mut container: HyperionContainer<TestMessage> =
        hyperion_container_factory::create::<DummyComponent, DummyConfig, TestMessage>(
            config_path.to_str().unwrap(),
            topology_path.to_str().unwrap(),
            container_state.clone(),
            container_state_notify.clone(),
            main_rx,
        )
        .await;

    // Run container
    tokio::spawn(async move {
        container.run().await;
    });

    // Give server and container some time, then shutdown
    sleep(Duration::from_millis(200)).await;
    main_tx
        .send(TestMessage::Framework(ContainerDirective::Shutdown))
        .await
        .unwrap();

    // Wait for Closed
    let deadline = tokio::time::Instant::now() + Duration::from_secs(6);
    loop {
        if container_state.load(Ordering::SeqCst) == ContainerState::Closed as usize {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("Timed out waiting for container to close via factory");
        }
        sleep(Duration::from_millis(50)).await;
    }
}
