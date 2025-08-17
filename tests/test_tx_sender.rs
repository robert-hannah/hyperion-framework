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
use std::time::Instant;

// Package
use hyperion_framework::utilities::tx_sender::add_to_tx_with_retry;
use tokio::sync::mpsc;
use tokio::time::Duration;

#[tokio::test]
async fn succeeds_immediately() {
    let (tx, mut rx) = mpsc::channel::<u32>(4);

    // Send a value
    add_to_tx_with_retry(&tx, &42u32, "test", "test").await;

    // Receive
    let got = rx.recv().await.expect("should receive");
    assert_eq!(got, 42);
}

#[tokio::test]
async fn retries_then_succeeds_after_capacity_frees() {
    let (tx, mut rx) = mpsc::channel::<u32>(1);

    // Fill the channel to force initial try_send to be Full
    tx.try_send(1).unwrap();

    // Spawn the retrying send in the background (owning a clone of tx and message)
    let tx_clone = tx.clone();
    let send_task = tokio::spawn(async move {
        let msg = 99u32;
        let start = Instant::now();
        add_to_tx_with_retry(&tx_clone, &msg, "from", "to").await;
        start.elapsed()
    });

    // After 150ms, consume one message to free capacity
    tokio::time::sleep(Duration::from_millis(150)).await;
    let _ = rx.recv().await; // remove the pre-filled value

    // Await the sender and check it took at least ~200ms (first backoff 200ms)
    let elapsed = send_task.await.expect("task join");
    assert!(
        elapsed >= Duration::from_millis(180),
        "elapsed: {:?}",
        elapsed
    );

    // Now the newly sent value should be available
    let got = rx.recv().await.expect("should receive second value");
    assert_eq!(got, 99);
}

#[tokio::test]
async fn closed_channel_returns_quickly() {
    let (tx, rx) = mpsc::channel::<u32>(1);
    drop(rx); // close the channel

    let start = Instant::now();
    add_to_tx_with_retry(&tx, &7u32, "from", "to").await;
    let elapsed = start.elapsed();

    // Should complete quickly (no sleeps on Closed error)
    assert!(
        elapsed < Duration::from_millis(50),
        "elapsed: {:?}",
        elapsed
    );
}
