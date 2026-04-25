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
use std::sync::Arc as StdArc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// Package
use hyperion_framework::containerisation::container_state::ContainerState;
use hyperion_framework::messages::container_directive::ContainerDirective;
use hyperion_framework::network::serialiser::serialise_message;
use hyperion_framework::network::server::Server;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, mpsc};
use tokio::time::{Duration, sleep};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ContainerMessage {
    ContainerDirectiveMsg(ContainerDirective),
}

// ---- Helpers ---------------------------------------------------------------------------------

/// Bind to port 0, let the OS assign an ephemeral port, drop the listener, and return the address.
/// The port is free again immediately after this returns, ready for Server::new() to rebind.
async fn ephemeral_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().to_string()
    // listener drops here, releasing the port
}

/// Simulate a client connecting to `address` and sending one framed ContainerDirective message.
async fn _client_task(id: usize, address: String) {
    let message = ContainerMessage::ContainerDirectiveMsg(ContainerDirective::RetryAllConnections);
    let payload = serialise_message(&message).expect("Message serialisation failed");
    let len_prefix = (payload.len() as u32).to_be_bytes();
    let mut framed = Vec::with_capacity(4 + payload.len());
    framed.extend_from_slice(&len_prefix);
    framed.extend_from_slice(&payload);

    match TcpStream::connect(&address).await {
        Ok(mut stream) => match stream.write_all(&framed).await {
            Ok(_) => log::debug!("Message sent to server! ID: {id}"),
            Err(e) => log::error!("Failed to send message: {e:?} ID: {id}"),
        },
        Err(e) => log::error!("Couldn't connect to {address}: {e:?} ID: {id}"),
    }
}

fn new_state_and_notify() -> (StdArc<AtomicUsize>, StdArc<Notify>) {
    (
        StdArc::new(AtomicUsize::new(ContainerState::Running as usize)),
        StdArc::new(Notify::new()),
    )
}

// ---- Tests -----------------------------------------------------------------------------------

#[tokio::test]
async fn test_server_high_loading() {
    let addr = ephemeral_addr().await;

    let (server_tx, rx) = mpsc::channel::<ContainerMessage>(120);
    let (container_state, container_state_notify) = new_state_and_notify();

    let arc_server = Server::new(
        addr.clone(),
        server_tx,
        container_state.clone(),
        container_state_notify.clone(),
    );

    tokio::spawn(async move {
        if let Err(e) = Server::run(arc_server).await {
            log::error!("Server error: {e:?}");
        }
    });

    // Give the server a moment to bind and start accepting.
    sleep(Duration::from_millis(50)).await;

    let mut handles = Vec::new();
    let start_time = Instant::now();
    let client_count = 100;

    for i in 0..client_count {
        let addr_clone = addr.clone();
        handles.push(tokio::spawn(
            async move { _client_task(i, addr_clone).await },
        ));
    }

    for handle in handles {
        handle.await.expect("client task");
    }

    // Give the server time to process all inbound messages.
    sleep(Duration::from_millis(500)).await;

    let duration = start_time.elapsed();
    log::debug!("Server strain test completed in {duration:?}");

    assert_eq!(
        rx.len(),
        client_count,
        "Some messages were not received by the server"
    );

    container_state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    container_state_notify.notify_waiters();
}

#[tokio::test]
async fn test_server_shutdown_command() {
    let addr = ephemeral_addr().await;

    let (server_tx, _rx) = mpsc::channel::<ContainerMessage>(10);
    let (container_state, container_state_notify) = new_state_and_notify();

    let arc_server = Server::new(
        addr,
        server_tx,
        container_state.clone(),
        container_state_notify.clone(),
    );

    let server_handle = tokio::spawn(async move {
        if let Err(e) = Server::run(arc_server).await {
            panic!("Server error: {e:?}");
        }
    });

    // Give the server time to bind.
    sleep(Duration::from_millis(50)).await;

    // Signal shutdown.
    container_state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    container_state_notify.notify_waiters();

    // The server task should exit cleanly.
    assert!(
        server_handle.await.is_ok(),
        "Server task did not terminate cleanly"
    );
}

#[tokio::test]
async fn test_server_does_not_blow_up_on_invalid_message_deserialisation() {
    let addr = ephemeral_addr().await;

    let (server_tx, _rx) = mpsc::channel::<ContainerMessage>(10);
    let (container_state, container_state_notify) = new_state_and_notify();

    let arc_server = Server::new(
        addr.clone(),
        server_tx,
        container_state.clone(),
        container_state_notify.clone(),
    );

    tokio::spawn(async move {
        if let Err(e) = Server::run(arc_server).await {
            log::error!("Server error: {e:?}");
        }
    });

    sleep(Duration::from_millis(50)).await;

    // Send garbage bytes — server must not crash.
    if let Ok(mut stream) = TcpStream::connect(&addr).await {
        let _ = stream.write_all(&[0xFF, 0xFF, 0xFF, 0xFF]).await;
    }

    sleep(Duration::from_millis(100)).await;

    assert_eq!(
        container_state.load(Ordering::SeqCst),
        ContainerState::Running as usize,
        "Server crashed after receiving an invalid message"
    );

    container_state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    container_state_notify.notify_waiters();
}
