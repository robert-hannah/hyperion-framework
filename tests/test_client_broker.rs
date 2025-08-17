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
use std::sync::{Arc as StdArc, atomic::{AtomicUsize, Ordering}};

// Package
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::time::{timeout, Duration, sleep};
use hyperion_framework::containerisation::client_broker::ClientBroker;
use hyperion_framework::containerisation::container_state::ContainerState;
use hyperion_framework::messages::client_broker_message::ClientBrokerMessage;
use hyperion_framework::network::network_topology::{NetworkTopology, ClientConnections, Connection};


async fn start_test_server() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("Bind test server");
    let addr = listener.local_addr().unwrap();
    (listener, addr.to_string())
}

async fn read_one_framed_string(listener: TcpListener) -> String {
    let (mut socket, _) = listener.accept().await.expect("Accept connection");

    // Read 4-byte big-endian length prefix
    let mut len_buf = [0u8; 4];
    socket.read_exact(&mut len_buf).await.expect("Read len");
    let len = u32::from_be_bytes(len_buf) as usize;

    // Read payload
    let mut payload = vec![0u8; len];
    socket.read_exact(&mut payload).await.expect("Read payload");

    // Deserialize JSON String
    serde_json::from_slice::<String>(&payload).expect("Deserialize string")
}

async fn read_two_framed_strings(listener: TcpListener) -> (String, Option<String>) {
    let (mut socket, _) = listener.accept().await.expect("Accept connection");

    // Helper to read one message from an established socket
    async fn read_msg(socket: &mut tokio::net::TcpStream) -> Option<String> {
        let mut len_buf = [0u8; 4];
        if socket.read_exact(&mut len_buf).await.is_err() { return None; }
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        if socket.read_exact(&mut payload).await.is_err() { return None; }
        Some(serde_json::from_slice::<String>(&payload).expect("Deserialize string"))
    }

    let first = read_msg(&mut socket).await.expect("First message");

    // Try to read a second message with a short timeout; if none, return None
    let second =
        timeout(Duration::from_millis(500), read_msg(&mut socket)).await.unwrap_or_else(|_| None);

    (first, second)
}

fn build_topology_with_client(name: &str, address: String) -> StdArc<NetworkTopology> {
    StdArc::new(NetworkTopology {
        container_name: "test-container".to_string(),
        server_address: "127.0.0.1:0".to_string(),
        client_connections: ClientConnections {
            client_connection_vec: vec![Connection { name: name.to_string(), address }],
        },
    })
}

fn new_state_and_notify() -> (StdArc<AtomicUsize>, StdArc<Notify>) {
    (StdArc::new(AtomicUsize::new(ContainerState::Running as usize)), StdArc::new(Notify::new()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handle_message_forwards_to_correct_client() {
    // Arrange: server and topology
    let (listener, addr) = start_test_server().await;
    let topology = build_topology_with_client("Alpha", addr.clone());

    let (container_state, container_notify) = new_state_and_notify();

    let broker = ClientBroker::<String>::init(topology, container_state.clone(), container_notify.clone());

    // Allow client time to connect
    sleep(Duration::from_millis(100)).await;

    // Act: send message to target client
    let msg = ClientBrokerMessage::new(vec!["Alpha"], "Beta".to_string());

    // Spawn reader before sending to ensure we catch it
    let read_task = tokio::spawn(read_one_framed_string(listener));

    broker.handle_message(msg).await;

    // Assert: server received exactly that string
    let received = timeout(Duration::from_secs(2), read_task).await.expect("No timeout").expect("Task ok");
    assert_eq!(received, "Beta");

    // Shutdown cleanly
    container_state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    container_notify.notify_waiters();
    // Give clients time to observe shutdown and exit
    // Mutable broker to call shutdown later; recreate mutable binding
    let mut broker = broker;
    broker.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_forward_shutdown_blocks_future_messages() {
    let (listener, addr) = start_test_server().await;
    let topology = build_topology_with_client("Alpha", addr.clone());
    let (container_state, container_notify) = new_state_and_notify();

    let mut broker = ClientBroker::<String>::init(topology, container_state.clone(), container_notify.clone());

    // Allow client to connect
    sleep(Duration::from_millis(100)).await;

    // Reader task to capture up to two messages
    let read_task = tokio::spawn(read_two_framed_strings(listener));

    // Forward_shutdown sends to client and then disables messaging
    broker.forward_shutdown("Beta".to_string()).await;

    // Try to send another message
    let later = ClientBrokerMessage::new(vec!["Alpha"], "Charlie".to_string());
    broker.handle_message(later).await;

    let (first, second) =
        timeout(Duration::from_secs(3), read_task).await.expect("No timeout").expect("Task ok");
    assert_eq!(first, "Beta");
    assert!(second.is_none(), "Expected no second message after forward_shutdown, got: {:?}", second);

    // Shutdown clients
    container_state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    container_notify.notify_waiters();
    broker.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_unknown_target_does_not_panic() {
    // Topology with no clients
    let topology = StdArc::new(NetworkTopology {
        container_name: "test".to_string(),
        server_address: "127.0.0.1:0".to_string(),
        client_connections: ClientConnections { client_connection_vec: vec![] },
    });

    let (container_state, container_notify) = new_state_and_notify();

    let broker = ClientBroker::<String>::init(topology, container_state, container_notify);

    // Should not panic even though target is unknown
    broker.handle_message(ClientBrokerMessage::new(vec!["Ghost"], "Msg".to_string())).await;

    // No clients to shutdown; ensure Drop path is fine by creating mutable to call shutdown
    let mut broker = broker;
    // No client_threads; should complete immediately
    broker.shutdown().await;
}
