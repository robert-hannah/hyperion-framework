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
use std::sync::Arc as StdArc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// Package
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};
use tokio::{
    sync::{Notify, mpsc},
    time::timeout,
};

// Local
use crate::containerisation::container_state::ContainerState;
use crate::network::client::Client;
use crate::network::serialiser;
use crate::test::mock_container_message::ContainerMessage;

// Constants
const _WAIT_TIME: u64 = 5;

#[tokio::test]
async fn test_client_connects_to_server() {
    let address = "127.0.0.1:9999";
    let (_tx, rx) = mpsc::channel::<ContainerMessage>(10);
    let state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let notify = StdArc::new(Notify::new());

    let listener = TcpListener::bind(address)
        .await
        .expect("Failed to bind server");
    let client = Client::new(
        "TestClient".to_string(),
        address.to_string(),
        rx,
        state.clone(),
        notify.clone(),
        3,
    );

    // Run the client in a separate task
    let client_task = tokio::spawn(async move {
        let _ = client.run().await;
    });

    // Accept the connection
    let (socket, _) = listener
        .accept()
        .await
        .expect("Failed to accept connection");

    // Wait a moment to ensure connections have stabilised
    tokio::time::sleep(Duration::from_secs(_WAIT_TIME)).await;

    assert!(socket.peer_addr().is_ok());

    // Cleanup
    state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    notify.notify_waiters();
    client_task.await.expect("Client task failed");
}

#[tokio::test]
async fn test_client_does_not_blow_up_on_connection_failure() {
    let address = "127.0.0.1:9998";
    let (_tx, rx) = mpsc::channel::<ContainerMessage>(10);
    let state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let notify = StdArc::new(Notify::new());

    let client = Client::new(
        "TestClient".to_string(),
        address.to_string(),
        rx,
        state.clone(),
        notify.clone(),
        2, // Small retry limit for test
    );

    // Run the client in a separate task
    let client_task = tokio::spawn(async move {
        let _ = client.run().await;
    });

    // Give some time for retries
    tokio::time::sleep(Duration::from_secs(_WAIT_TIME)).await;

    // Cleanup - should do it itself due to connection failure
    client_task.await.expect("Client task failed");
}

#[tokio::test]
async fn test_client_retries_on_server_connection_failure() {
    let address = "127.0.0.1:9997";
    let (tx, rx) = mpsc::channel::<String>(10);
    let state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let notify = StdArc::new(Notify::new());

    let client = Client::new(
        "TestClient".to_string(),
        address.to_string(),
        rx,
        state.clone(),
        notify.clone(),
        10, // Allow retries
    );

    let client_task = tokio::spawn(async move {
        let _ = client.run().await;
    });

    // Wait a moment to let the client struggle
    tokio::time::sleep(Duration::from_secs(_WAIT_TIME)).await;

    // Start the server
    let listener = TcpListener::bind(address)
        .await
        .expect("Failed to bind server");
    tokio::time::sleep(Duration::from_secs(_WAIT_TIME)).await;

    // Create listener in server
    let (mut socket, _) = listener
        .accept()
        .await
        .expect("Failed to accept connection");
    tokio::time::sleep(Duration::from_secs(_WAIT_TIME)).await;

    // Send a test message
    let message = "Hello, world!".to_string();
    tx.send(message.clone())
        .await
        .expect("Failed to send message");

    let mut buffer = vec![0u8; 1024];
    let bytes_read = socket
        .read(&mut buffer)
        .await
        .expect("Failed to read message");

    // Step 1: Ensure we have at least 4 bytes for the length prefix
    assert!(
        bytes_read >= 4,
        "Expected at least 4 bytes for length prefix"
    );

    // Step 2: Parse length prefix
    let len_bytes: [u8; 4] = buffer[..4].try_into().expect("Failed to get length prefix");
    let msg_len = u32::from_be_bytes(len_bytes) as usize;

    // Step 3: Ensure the full message was received
    assert!(
        bytes_read >= 4 + msg_len,
        "Incomplete message: expected {} bytes, got {}",
        msg_len,
        bytes_read - 4
    );

    // Step 4: Extract and deserialise
    let msg_bytes = &buffer[4..4 + msg_len];
    let received_message: String =
        serialiser::deserialise_message(msg_bytes).expect("Deserialisation failed");
    assert_eq!(received_message, message);

    // Cleanup
    state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    notify.notify_waiters();
    client_task.await.expect("Client task panicked");
}

#[tokio::test]
async fn test_client_is_restartable() {
    let address = "127.0.0.1:9996";
    let (tx, rx1) = mpsc::channel::<String>(10);
    let state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let notify = StdArc::new(Notify::new());

    let listener = TcpListener::bind(address)
        .await
        .expect("Failed to bind server");
    let client = Client::new(
        "TestClient".to_string(),
        address.to_string(),
        rx1,
        state.clone(),
        notify.clone(),
        3,
    );

    let client_task = tokio::spawn(async move {
        let _ = client.run().await;
    });

    // Create listener in server
    let (mut socket, _) = listener
        .accept()
        .await
        .expect("Failed to accept connection");
    tokio::time::sleep(Duration::from_secs(_WAIT_TIME)).await;

    // Send a test message
    let message = "Hello, world!".to_string();
    tx.send(message.clone())
        .await
        .expect("Failed to send message");

    let mut buffer = vec![0u8; 1024];
    let bytes_read = socket
        .read(&mut buffer)
        .await
        .expect("Failed to read message");

    // Step 1: Ensure we have at least 4 bytes for the length prefix
    assert!(
        bytes_read >= 4,
        "Expected at least 4 bytes for length prefix"
    );

    // Step 2: Parse length prefix
    let len_bytes: [u8; 4] = buffer[..4].try_into().expect("Failed to get length prefix");
    let msg_len = u32::from_be_bytes(len_bytes) as usize;

    // Step 3: Ensure the full message was received
    assert!(
        bytes_read >= 4 + msg_len,
        "Incomplete message: expected {} bytes, got {}",
        msg_len,
        bytes_read - 4
    );

    // Step 4: Extract and deserialise
    let msg_bytes = &buffer[4..4 + msg_len];
    let received_message: String =
        serialiser::deserialise_message(msg_bytes).expect("Deserialisation failed");
    assert_eq!(received_message, message);

    // Shut down client
    state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    notify.notify_waiters();
    client_task
        .await
        .expect("Client did not shut down gracefully");

    // Create a new client to restart connection on same address
    // This is because the previous client was lost when moved to a new thread
    state.store(ContainerState::Running as usize, Ordering::SeqCst);
    let (tx2, rx2) = mpsc::channel::<String>(10);
    let client2 = Client::new(
        "TestClient".to_string(),
        address.to_string(),
        rx2,
        state.clone(),
        notify.clone(),
        3,
    );

    let restarted_task = tokio::spawn(async move {
        let _ = client2.run().await;
    });

    // Reinstate listener in server
    let (mut socket, _) = listener
        .accept()
        .await
        .expect("Failed to accept connection");
    tokio::time::sleep(Duration::from_secs(_WAIT_TIME)).await;

    // Send a test message
    let message2 = "Hello, world again!".to_string();
    tx2.send(message2.clone())
        .await
        .expect("Failed to send message");

    let mut buffer = vec![0u8; 1024];
    let bytes_read = socket
        .read(&mut buffer)
        .await
        .expect("Failed to read message");

    // Step 1: Ensure we have at least 4 bytes for the length prefix
    assert!(
        bytes_read >= 4,
        "Expected at least 4 bytes for length prefix"
    );

    // Step 2: Parse length prefix
    let len_bytes: [u8; 4] = buffer[..4].try_into().expect("Failed to get length prefix");
    let msg_len = u32::from_be_bytes(len_bytes) as usize;

    // Step 3: Ensure the full message was received
    assert!(
        bytes_read >= 4 + msg_len,
        "Incomplete message: expected {} bytes, got {}",
        msg_len,
        bytes_read - 4
    );

    // Step 4: Extract and deserialise
    let msg_bytes = &buffer[4..4 + msg_len];
    let received_message2: String =
        serialiser::deserialise_message(msg_bytes).expect("Deserialisation failed");

    assert_eq!(received_message2, message2);

    // Shut down client
    state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    notify.notify_waiters();
    restarted_task
        .await
        .expect("Client did not shut down gracefully");
}

#[tokio::test]
async fn test_client_sends_messages() {
    let address = "127.0.0.1:9995";
    let (tx, rx) = mpsc::channel::<String>(10);
    let state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let notify = StdArc::new(Notify::new());

    let listener = TcpListener::bind(address)
        .await
        .expect("Failed to bind server");
    let client = Client::new(
        "TestClient".to_string(),
        address.to_string(),
        rx,
        state.clone(),
        notify.clone(),
        3,
    );

    let client_task = tokio::spawn(async move {
        let _ = client.run().await;
    });

    // Create listener in server
    let (mut socket, _) = listener
        .accept()
        .await
        .expect("Failed to accept connection");
    tokio::time::sleep(Duration::from_secs(_WAIT_TIME)).await;

    // Send a test message
    let message = "Hello, world!".to_string();
    tx.send(message.clone())
        .await
        .expect("Failed to send message");

    let mut buffer = vec![0u8; 1024];
    let bytes_read = socket
        .read(&mut buffer)
        .await
        .expect("Failed to read message");

    // Step 1: Ensure we have at least 4 bytes for the length prefix
    assert!(
        bytes_read >= 4,
        "Expected at least 4 bytes for length prefix"
    );

    // Step 2: Parse length prefix
    let len_bytes: [u8; 4] = buffer[..4].try_into().expect("Failed to get length prefix");
    let msg_len = u32::from_be_bytes(len_bytes) as usize;

    // Step 3: Ensure the full message was received
    assert!(
        bytes_read >= 4 + msg_len,
        "Incomplete message: expected {} bytes, got {}",
        msg_len,
        bytes_read - 4
    );

    // Step 4: Extract and deserialise
    let msg_bytes = &buffer[4..4 + msg_len];
    let received_message: String =
        serialiser::deserialise_message(msg_bytes).expect("Deserialisation failed");
    assert_eq!(received_message, message);

    // Cleanup
    state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    notify.notify_waiters();
    client_task.await.expect("Client task failed");
}

#[tokio::test]
async fn test_client_shuts_down_gracefully() {
    let address = "127.0.0.1:9994";
    let listener = TcpListener::bind(address)
        .await
        .expect("Failed to bind server");

    let (_tx, rx) = mpsc::channel::<ContainerMessage>(10);
    let state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    let notify = StdArc::new(Notify::new());

    let client = Client::new(
        "TestClient".to_string(),
        address.to_string(),
        rx,
        state.clone(),
        notify.clone(),
        3,
    );

    let client_task = tokio::spawn(async move {
        let _ = client.run().await;
    });

    let (_socket, _) = listener
        .accept()
        .await
        .expect("Failed to accept connection");

    // Wait a moment to ensure connections have stabilised
    tokio::time::sleep(Duration::from_secs(_WAIT_TIME)).await;

    // Initiate shutdown
    state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    notify.notify_waiters();

    // Wait for the client to shut down
    let result = client_task.await;
    assert!(result.is_ok(), "Client did not shut down gracefully");
}

#[tokio::test]
async fn test_client_does_not_blow_up_on_invaid_message_serialisation() {
    // TODO: I'm not entiirely sure how to do this...
    // tx enforces the type and the client enforces that the type is serialisable
    // Which means I'd have to directly corrupt the message without losing its type?

    // let address = "127.0.0.1:9993";
    // let listener = TcpListener::bind(address).await.expect("Failed to bind server");
    // let (tx, rx) = mpsc::channel::<ContainerMessage>(10);
    // let state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    // let notify = StdArc::new(Notify::new());
    // let client = Client::new(
    //     "TestClient".to_string(),
    //     address.to_string(),
    //     rx,
    //     state.clone(),
    //     notify.clone(),
    //     3,
    // );
    // let client_task = tokio::spawn(async move {
    //     let _ = client.run().await;
    // });
    // let (_socket, _) = listener.accept().await.expect("Failed to accept connection");
    // tokio::time::sleep(Duration::from_secs(5)).await;
    // // Send invalid data
    // let invalid_message = "Bosh";
    // tx.send(invalid_message).await.expect("Failed to send message");
    // // Ensure client does not crash
    // tokio::time::sleep(Duration::from_secs(5)).await;
    // state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    // notify.notify_waiters();
    // client_task.await.expect("Client task panicked");
}

#[tokio::test]
async fn test_client_retries_on_invaid_message_serialisation() {
    // See above

    // let address = "127.0.0.1:9992";
    // let listener = TcpListener::bind(address).await.expect("Failed to bind server");
    // let (tx, rx) = mpsc::channel::<ContainerMessage>(10);
    // let state = StdArc::new(AtomicUsize::new(ContainerState::Running as usize));
    // let notify = StdArc::new(Notify::new());
    // let client = Client::new(
    //     "TestClient".to_string(),
    //     address.to_string(),
    //     rx,
    //     state.clone(),
    //     notify.clone(),
    //     3,
    // );
    // let client_task = tokio::spawn(async move {
    //     let _ = client.run().await;
    // });
}
