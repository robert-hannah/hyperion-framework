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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc as StdArc;

// Package
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Notify};
use tokio::time::{sleep, Duration};
use hyperion_framework::containerisation::container_state::ContainerState;
use hyperion_framework::containerisation::hyperion_container::HyperionContainer;
use hyperion_framework::containerisation::traits::{HyperionContainerDirectiveMessage, Run};
use hyperion_framework::messages::client_broker_message::ClientBrokerMessage;
use hyperion_framework::messages::container_directive::ContainerDirective;
use hyperion_framework::containerisation::client_broker::ClientBroker;
use hyperion_framework::network::network_topology::{ClientConnections, NetworkTopology};


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum TestMessage {
    Framework(ContainerDirective),
    UserIncrement,
}

impl HyperionContainerDirectiveMessage for TestMessage {
    fn get_container_directive_message(&self) -> Option<&ContainerDirective> {
        if let TestMessage::Framework(d) = self { Some(d) } else { None }
    }
}

#[derive(Debug, Clone)]
struct DummyComponent;

#[async_trait::async_trait]
impl Run for DummyComponent {
    type Message = TestMessage;
    async fn run(self, mut comp_in_rx: mpsc::Receiver<Self::Message>, _comp_out_tx: mpsc::Sender<ClientBrokerMessage<Self::Message>>) {
        // Simple loop that increments a static counter on UserIncrement and exits on Shutdown
        loop {
            if let Some(msg) = comp_in_rx.recv().await {
                match msg {
                    TestMessage::Framework(ContainerDirective::Shutdown) => break,
                    TestMessage::UserIncrement => {
                        TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }
    }
}

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn empty_client_broker<T>(
    container_state: StdArc<AtomicUsize>,
    container_state_notify: StdArc<Notify>,
) -> ClientBroker<T> 
where T:
    std::fmt::Debug 
    + Send 
    + 'static 
    + for<'de> Deserialize<'de> 
    + Sync 
    + Clone 
    + Serialize
{
    let topology = StdArc::new(NetworkTopology {
        container_name: "test".into(),
        server_address: "127.0.0.1:0".into(),
        client_connections: ClientConnections { client_connection_vec: vec![] },
    });
    ClientBroker::init(topology, container_state, container_state_notify)
}

#[tokio::test]
async fn forwards_non_framework_messages_to_component() {
    let container_state: StdArc<AtomicUsize> = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let container_state_notify: StdArc<Notify> = StdArc::new(Notify::new());

    let (main_tx, main_rx) = mpsc::channel::<TestMessage>(32);
    let (_server_tx, server_rx) = mpsc::channel::<TestMessage>(32);

    let client_broker = empty_client_broker::<TestMessage>(container_state.clone(), container_state_notify.clone());

    let mut container = HyperionContainer::create(
        DummyComponent,
        container_state.clone(),
        container_state_notify.clone(),
        client_broker,
        main_rx,
        server_rx,
    );

    // spawn container
    tokio::spawn(async move {
        container.run().await;
    });

    TEST_COUNTER.store(0, Ordering::SeqCst);

    // Send a non-framework message and allow it to propagate
    main_tx.send(TestMessage::UserIncrement).await.unwrap();
    sleep(Duration::from_millis(200)).await;

    assert_eq!(TEST_COUNTER.load(Ordering::SeqCst), 1);

    // Shutdown the container to clean up
    main_tx.send(TestMessage::Framework(ContainerDirective::Shutdown)).await.unwrap();

    // Wait for container to transition to Closed (run sleeps 3 seconds during shutdown)
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if container_state.load(Ordering::SeqCst) == ContainerState::Closed as usize { break; }
        if tokio::time::Instant::now() > deadline { panic!("timeout waiting for container to close"); }
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn shutdown_and_system_shutdown_transitions_state() {
    let container_state: StdArc<AtomicUsize> = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let container_state_notify: StdArc<Notify> = StdArc::new(Notify::new());

    let (main_tx, main_rx) = mpsc::channel::<TestMessage>(32);
    let (_server_tx, server_rx) = mpsc::channel::<TestMessage>(32);

    let client_broker = empty_client_broker::<TestMessage>(container_state.clone(), container_state_notify.clone());

    let mut container = HyperionContainer::create(
        DummyComponent,
        container_state.clone(),
        container_state_notify.clone(),
        client_broker,
        main_rx,
        server_rx,
    );

    let handle = tokio::spawn(async move {
        container.run().await;
    });

    // Send SystemShutdown to exercise forward_shutdown + shutdown path
    main_tx.send(TestMessage::Framework(ContainerDirective::SystemShutdown)).await.unwrap();

    // Expect Closed state within a reasonable time (3.5s should be enough: 0.5s broker + loop delay + 3s shutdown sleep)
    let deadline = tokio::time::Instant::now() + Duration::from_secs(6);
    loop {
        if container_state.load(Ordering::SeqCst) == ContainerState::Closed as usize { break; }
        if tokio::time::Instant::now() > deadline { panic!("timeout waiting for container to close after system shutdown"); }
        sleep(Duration::from_millis(50)).await;
    }

    // Ensure task exits
    handle.await.unwrap();
}
