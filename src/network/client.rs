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
use std::fmt::Debug;
use std::sync::{
    Arc as StdArc,
    atomic::{AtomicUsize, Ordering},
};

// Package
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::{Notify, mpsc};
use tokio::time::{Duration, sleep};

// Local
use crate::containerisation::container_state::ContainerState;
use crate::network::serialiser;

pub struct Client<T> {
    pub connection_name: String,
    server_address: String,
    client_rx: mpsc::Receiver<T>,
    container_state: StdArc<AtomicUsize>,
    container_state_notify: StdArc<Notify>,
    send_retries: u8,
    max_send_retries: u8,
    internal_client_state: bool,
}

// ========================================================================
//    A Client will ONLY send messages
// ========================================================================

impl<T> Client<T>
where
    T: Clone + Serialize + Debug,
{
    // ========================================================================
    //    Create new Client
    // ========================================================================
    pub fn new(
        connection_name: String,
        server_address: String,
        client_rx: mpsc::Receiver<T>,
        container_state: StdArc<AtomicUsize>,
        container_state_notify: StdArc<Notify>,
        max_send_retries: u8,
    ) -> Self {
        Self {
            connection_name,
            server_address,
            client_rx,
            container_state,
            container_state_notify,
            send_retries: 0,
            max_send_retries,
            internal_client_state: true, // Default to true, Client is operational
        }
    }

    // ========================================================================
    //    Run async Client
    // ========================================================================
    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.internal_client_state = true;
        self.send_retries = 0;

        loop {
            // This check must be here or the client will just keep restarting if message send failure
            if self.is_closing_connection() {
                break;
            }

            match TcpStream::connect(self.server_address.to_string()).await {
                Ok(mut stream) => {
                    log::info!(
                        "{} Client connected on {}",
                        self.connection_name,
                        self.server_address
                    );
                    self.send_retries = 0; // Reset retry counter after successful connection

                    loop {
                        tokio::select! {
                            // Listen for incoming messages from tokio mpsc channel
                            // Using select and tokio as messages can be sent instantly
                            Some(message) = self.client_rx.recv() => {
                                let payload = match serialiser::serialise_message(&message) {
                                    Ok(payload) => payload,
                                    Err(e) => {
                                        log::error!("Failed to serialise message: {e:?} \n{message:?}");
                                        continue; // Skip this message and try the next one
                                    }
                                };

                                // Length-prefix the payload
                                let len = (payload.len() as u32).to_be_bytes(); // 4-byte big-endian length
                                let mut framed_msg = Vec::with_capacity(4 + payload.len());
                                framed_msg.extend_from_slice(&len);
                                framed_msg.extend_from_slice(&payload);

                                // Send message
                                match stream.write_all(&framed_msg).await {
                                    Ok(_) => {
                                        log::trace!("Message sent: {message:?}");
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to send message to {}: {e:?}", self.connection_name);
                                        self.send_retries += 1;

                                        if self.send_retries >= self.max_send_retries {
                                            log::warn!("{} Client: {} failed to send message after {} retries. Closing client...",
                                                self.connection_name, self.server_address, self.max_send_retries);
                                            self.internal_client_state = false;

                                            // Close stream
                                            if let Err(e) = stream.shutdown().await {
                                                log::warn!("Failed to shutdown stream: {e:?}");
                                            }
                                            break;
                                        }

                                        // Halved exponential backoff before retrying connection
                                        sleep(Duration::from_secs((2u64.pow(self.send_retries as u32)) / 2)).await;
                                    }
                                }
                            },

                            // Listen for state changes
                            _ = self.container_state_notify.notified() => {
                                // Check if state has been set to ShuttingDown
                                if self.is_closing_connection() {
                                    // Close the stream
                                    if let Err(e) = stream.shutdown().await {
                                        log::warn!("Failed to shutdown stream: {e:?}");
                                    }
                                    break; // Break out of receiving loop
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    // Connection to Server failed - retry
                    log::warn!(
                        "{} Client: {} failed to connect: {e:?}",
                        self.connection_name,
                        self.server_address
                    );
                    self.send_retries += 1;

                    if self.send_retries >= self.max_send_retries {
                        log::warn!(
                            "Client {} failed to connect to Server after {} retries",
                            self.server_address,
                            self.max_send_retries
                        );
                        self.internal_client_state = false;
                        break;
                    }

                    // Halved exponential backoff before retrying connection
                    sleep(Duration::from_secs(
                        (2u64.pow(self.send_retries as u32)) / 2,
                    ))
                    .await;
                }
            }
        }

        log::info!(
            "{} Client: {} closed gracefully",
            self.connection_name,
            self.server_address
        );
        Ok(())
    }

    fn is_closing_connection(&self) -> bool {
        if ContainerState::from(self.container_state.load(Ordering::SeqCst))
            == ContainerState::ShuttingDown
        {
            log::info!(
                "{} Client ({}) has received closing instruction via ContainerState...",
                self.connection_name,
                self.server_address
            );
            true
        } else if !self.internal_client_state {
            log::info!(
                "{} Client ({}) is closing due to connection issues...",
                self.connection_name,
                self.server_address
            );
            true
        } else {
            false
        }
    }
}
