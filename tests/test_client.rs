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

// Package
use hyperion_framework::containerisation::container_state::ContainerState;
use hyperion_framework::messages::container_directive::ContainerDirective;
use hyperion_framework::network::client::Client;
use hyperion_framework::network::serialiser;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::{Notify, mpsc};
use tokio::time::{Duration, sleep};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ContainerMessage {
    ContainerDirectiveMsg(ContainerDirective),
}

// ---- Helpers ---------------------------------------------------------------------------------

/// Bind to port 0, let the OS assign an ephemeral port, and return the listener + its address.
async fn bind_ephemeral() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ephemeral listener");
    let addr = listener.local_addr().unwrap().to_string();
    (listener, addr)
}

/// Read one length-prefixed framed String message from an already-connected socket.
async fn read_framed_string(socket: &mut tokio::net::TcpStream) -> String {
    let mut len_buf = [0u8; 4];
    socket.read_exact(&mut len_buf).await.expect("read length prefix");
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    socket.read_exact(&mut payload).await.expect("read payload");
    serialiser::deserialise_message::<String>(&payload).expect("deserialise")
}

fn new_state_and_notify() -> (StdArc<AtomicUsize>, StdArc<Notify>) {
    (
        StdArc::new(AtomicUsize::new(ContainerState::Running as usize)),
        StdArc::new(Notify::new()),
    )
}

fn shutdown(state: &StdArc<AtomicUsize>, notify: &StdArc<Notify>) {
    state.store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
    notify.notify_waiters();
}

// ---- Tests -----------------------------------------------------------------------------------

#[tokio::test]
async fn test_client_connects_to_server() {
    let (listener, addr) = bind_ephemeral().await;
    let (state, notify) = new_state_and_notify();
    let (_tx, rx) = mpsc::channel::<ContainerMessage>(10);

    let client = Client::new("TestClient".into(), addr, rx, state.clone(), notify.clone(), 3);
    let client_task = tokio::spawn(async move { let _ = client.run().await; });

    let (socket, _) = listener.accept().await.expect("accept connection");
    assert!(socket.peer_addr().is_ok());

    // Give the client task time to reach its select loop before we call notify_waiters().
    // notify_waiters() only wakes tasks already polling notified() — it does not store a permit.
    sleep(Duration::from_millis(100)).await;

    shutdown(&state, &notify);
    client_task.await.expect("client task");
}

#[tokio::test]
async fn test_client_does_not_blow_up_on_connection_failure() {
    // Pre-bind then drop so the port is genuinely closed when the client tries.
    let (listener, addr) = bind_ephemeral().await;
    drop(listener);

    let (state, notify) = new_state_and_notify();
    let (_tx, rx) = mpsc::channel::<ContainerMessage>(10);

    // max_send_retries = 1: first failure increments to 1 >= 1, exits immediately with no sleep.
    let client = Client::new("TestClient".into(), addr, rx, state.clone(), notify.clone(), 1);
    let client_task = tokio::spawn(async move { let _ = client.run().await; });

    client_task.await.expect("client should exit cleanly after exhausting retries");
}

#[tokio::test]
async fn test_client_retries_on_server_connection_failure() {
    // Pre-bind to get an ephemeral address, then drop so the client's first attempt fails.
    let (tmp_listener, addr) = bind_ephemeral().await;
    drop(tmp_listener);

    let (tx, rx) = mpsc::channel::<String>(10);
    let (state, notify) = new_state_and_notify();

    let client = Client::new("TestClient".into(), addr.clone(), rx, state.clone(), notify.clone(), 10);
    let client_task = tokio::spawn(async move { let _ = client.run().await; });

    // Wait long enough for the first connect attempt to fail and begin its 1-second backoff.
    sleep(Duration::from_millis(200)).await;

    // Now bring the server up so the client can succeed on its next attempt (~1s from now).
    let listener = TcpListener::bind(&addr).await.expect("re-bind server");

    let (mut socket, _) = listener.accept().await.expect("accept connection");

    let message = "Hello, retry world!".to_string();
    tx.send(message.clone()).await.expect("send message");

    let received = read_framed_string(&mut socket).await;
    assert_eq!(received, message);

    shutdown(&state, &notify);
    client_task.await.expect("client task");
}

#[tokio::test]
async fn test_client_sends_messages() {
    let (listener, addr) = bind_ephemeral().await;
    let (tx, rx) = mpsc::channel::<String>(10);
    let (state, notify) = new_state_and_notify();

    let client = Client::new("TestClient".into(), addr, rx, state.clone(), notify.clone(), 3);
    let client_task = tokio::spawn(async move { let _ = client.run().await; });

    let (mut socket, _) = listener.accept().await.expect("accept connection");
    sleep(Duration::from_millis(100)).await;

    let message = "Hello, world!".to_string();
    tx.send(message.clone()).await.expect("send message");

    let received = read_framed_string(&mut socket).await;
    assert_eq!(received, message);

    shutdown(&state, &notify);
    client_task.await.expect("client task");
}

#[tokio::test]
async fn test_client_shuts_down_gracefully() {
    let (listener, addr) = bind_ephemeral().await;
    let (state, notify) = new_state_and_notify();
    let (_tx, rx) = mpsc::channel::<ContainerMessage>(10);

    let client = Client::new("TestClient".into(), addr, rx, state.clone(), notify.clone(), 3);
    let client_task = tokio::spawn(async move { let _ = client.run().await; });

    let (_socket, _) = listener.accept().await.expect("accept connection");
    sleep(Duration::from_millis(100)).await;

    shutdown(&state, &notify);
    assert!(client_task.await.is_ok(), "client did not shut down gracefully");
}

#[tokio::test]
async fn test_client_is_restartable() {
    let (listener, addr) = bind_ephemeral().await;
    let (state, notify) = new_state_and_notify();

    // --- First client ---
    let (tx1, rx1) = mpsc::channel::<String>(10);
    let client1 = Client::new("TestClient".into(), addr.clone(), rx1, state.clone(), notify.clone(), 3);
    let task1 = tokio::spawn(async move { let _ = client1.run().await; });

    let (mut socket1, _) = listener.accept().await.expect("accept first connection");
    sleep(Duration::from_millis(100)).await;

    tx1.send("Hello, world!".into()).await.expect("send");
    assert_eq!(read_framed_string(&mut socket1).await, "Hello, world!");

    shutdown(&state, &notify);
    task1.await.expect("first client task");

    // --- Second client (restart) ---
    state.store(ContainerState::Running as usize, Ordering::SeqCst);
    let (tx2, rx2) = mpsc::channel::<String>(10);
    let client2 = Client::new("TestClient".into(), addr, rx2, state.clone(), notify.clone(), 3);
    let task2 = tokio::spawn(async move { let _ = client2.run().await; });

    let (mut socket2, _) = listener.accept().await.expect("accept second connection");
    sleep(Duration::from_millis(100)).await;

    tx2.send("Hello, world again!".into()).await.expect("send");
    assert_eq!(read_framed_string(&mut socket2).await, "Hello, world again!");

    shutdown(&state, &notify);
    task2.await.expect("second client task");
}
