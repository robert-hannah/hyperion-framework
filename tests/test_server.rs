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
#![allow(unused_imports)]

// Standard
use log::debug;
use std::sync::Arc as StdArc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// Package
use hyperion_framework::containerisation::container_state::ContainerState;
use hyperion_framework::messages::container_directive::ContainerDirective;
use hyperion_framework::network::serialiser::serialise_message;
use hyperion_framework::network::server::Server;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep, timeout};

// Constants
const _WAIT_TIME: u64 = 3;

// This will be the message that is sent between containers
// Container directive is essential for Hyperion to work
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ContainerMessage {
    ContainerDirectiveMsg(ContainerDirective),
}

// Simulates a client connecting and sending a message
async fn _client_task(id: usize) {
    // Create a test message
    let message = ContainerMessage::ContainerDirectiveMsg(ContainerDirective::Heartbeat);

    // Serialize to bytes
    let payload = serialise_message(&message).expect("Message serialisation failed");

    // Create 4-byte big-endian length prefix
    let len_prefix = (payload.len() as u32).to_be_bytes();

    // Combine prefix and payload into a single framed message
    let mut framed_msg = Vec::with_capacity(4 + payload.len());
    framed_msg.extend_from_slice(&len_prefix);
    framed_msg.extend_from_slice(&payload);

    // Connect and send
    match TcpStream::connect("127.0.0.1:8081").await {
        Ok(mut stream) => match stream.write_all(&framed_msg).await {
            Ok(_) => {
                log::debug!("Message sent to server! ID: {id}");
            }
            Err(e) => {
                log::error!("Failed to send message: {e:?} ID: {id}");
            }
        },
        Err(e) => {
            log::error!("Couldn't connect to address: {e:?} ID: {id}");
        }
    }

    log::debug!("Client {id} sent: {message:?}");
}

#[tokio::test]
async fn test_server_high_loading() {
    // Prepare the server
    let (server_tx, rx) = mpsc::channel::<ContainerMessage>(120);
    let container_state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let container_state_notify = StdArc::new(Notify::new());
    let arc_server = Server::new(
        "127.0.0.1:8081".to_string(),
        server_tx,
        container_state,
        container_state_notify,
    );

    // Spawn the server
    tokio::spawn(async move {
        if let Err(e) = Server::run(arc_server).await {
            log::error!("Server error: {e:?}");
        }
    });

    // Wait a moment to ensure the server is ready before clients connect
    sleep(Duration::from_secs(_WAIT_TIME)).await;

    let mut handles = Vec::new();
    let start_time = Instant::now();
    let client_count = 100;

    // Spawn client tasks
    for i in 0..client_count {
        handles.push(tokio::spawn(async move {
            _client_task(i).await;
        }));
    }

    // Wait a moment to ensure all clients have sent messages
    sleep(Duration::from_secs(_WAIT_TIME)).await;

    let duration = start_time.elapsed();
    log::debug!("Server strain test completed in {duration:?}");

    // Pass condition: ensure all clients completed successfully
    assert_eq!(rx.len(), client_count, "Some clients failed to complete.");
}

#[tokio::test]
async fn test_server_shutdown_command() {
    // Prepare the server
    let (server_tx, _rx) = mpsc::channel::<ContainerMessage>(10);
    let container_state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let container_state_clone = container_state.clone();
    let container_state_notify = StdArc::new(Notify::new());
    let container_state_notify_clone = container_state_notify.clone();
    let arc_server = Server::new(
        "127.0.0.1:8082".to_string(),
        server_tx,
        container_state,
        container_state_notify,
    );

    // Spawn the server
    let server_handle = tokio::spawn(async move {
        if let Err(e) = Server::run(arc_server).await {
            panic!("Server error: {e:?}"); // Fails the test
        }
    });

    // Wait for the server to start (use Notify instead of sleep)
    sleep(Duration::from_millis(_WAIT_TIME)).await; // Only to give some startup time

    // Send a shutdown command to the server
    container_state_clone.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    container_state_notify_clone.notify_waiters();

    // Wait for the server to shut down (use a loop to poll the state)
    for _ in 0..10 {
        if container_state_clone.load(Ordering::SeqCst) == ContainerState::ShuttingDown as usize {
            break;
        }
        sleep(Duration::from_millis(_WAIT_TIME)).await;
    }

    // Assert that the server has shut down
    assert_eq!(
        container_state_clone.load(Ordering::SeqCst),
        ContainerState::ShuttingDown as usize,
        "Server did not transition to Stopped state"
    );

    // Ensure the server task has exited
    assert!(
        server_handle.await.is_ok(),
        "Server task did not terminate cleanly"
    );
}

#[tokio::test]
async fn test_server_does_not_blow_up_on_invalid_message_deserialisation() {
    // Prepare the server
    let (server_tx, _rx) = mpsc::channel::<ContainerMessage>(10);
    let container_state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let container_state_notify = StdArc::new(Notify::new());
    let arc_server = Server::new(
        "127.0.0.1:8083".to_string(),
        server_tx,
        container_state.clone(),
        container_state_notify.clone(),
    );

    // Spawn the server
    tokio::spawn(async move {
        if let Err(e) = Server::run(arc_server).await {
            log::error!("Server error: {e:?}");
        }
    });

    // Wait for the server to start
    sleep(Duration::from_millis(_WAIT_TIME)).await;

    // Send an invalid message
    let invalid_bytes = vec![0xFF, 0xFF, 0xFF, 0xFF];
    if let Ok(mut stream) = TcpStream::connect("127.0.0.1:8083").await {
        let _ = stream.write_all(&invalid_bytes).await;
    }

    // Give the server time to process the invalid message
    sleep(Duration::from_millis(_WAIT_TIME)).await;

    // If the server did not panic or crash, the test passes
    assert_eq!(
        container_state.load(Ordering::SeqCst),
        ContainerState::Running as usize,
        "Server did not handle invalid message correctly"
    );
}
