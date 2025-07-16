// -------------------------------------------------------------------------------------------------
// Hyperion Framework
// https://github.com/Bazzz-1/hyperion-framework
//
// A lightweight Rust framework for building modular, component-based systems
// with built-in TCP messaging and CLI control.
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
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinSet;

// Local
use crate::containerisation::container_state::ContainerState;
use crate::network::serialiser;
use crate::utilities::tx_sender::add_to_tx_with_retry;

#[derive(Debug, Clone)]
pub struct Server<T> {
    address: String,
    server_tx: mpsc::Sender<T>,
    container_state: StdArc<AtomicUsize>,
    container_state_notify: StdArc<Notify>,
}

// ========================================================================
//    A Server will ONLY receive messages
// ========================================================================

impl<T> Server<T>
where
    T: Clone + Send + Serialize + DeserializeOwned + Sync + 'static,
{
    // ========================================================================
    //    Create new Server
    // ========================================================================
    pub fn new(
        address: String,
        server_tx: mpsc::Sender<T>,
        container_state: StdArc<AtomicUsize>,
        container_state_notify: StdArc<Notify>,
    ) -> StdArc<Self> {
        StdArc::new(Self {
            address,
            server_tx,
            container_state,
            container_state_notify,
        })
    }

    // These functions are static as they only handle StdArc<Server<T>>. This is because self is a
    // server and not a StdArc<Server<T>>.
    // I think using StdArc on objects for multithreaded async tasks is overall a good idea though,
    // maintains atomicity. It's just a bit of an ugly implementation.

    // ========================================================================
    //    STATIC: Run multi-thread async Server
    // ========================================================================
    pub async fn run(arc_server: StdArc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(arc_server.address.clone()).await?; // Connect to port
        log::trace!("Server listening on {}", arc_server.address);

        let mut join_set = JoinSet::new(); // Track spawned tasks

        loop {
            tokio::select! {     // Loop fires on incoming messages from listener or shutdown signal
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            log::info!("Accepted connection from {addr}");
                            // Clones an Arc pointer, increasing the reference count instead of doing a deep copy of the class
                            // Doing this as String and Sender are expensive to clone and StdArc::clone is far more atomic
                            let handler = StdArc::clone(&arc_server);
                            join_set.spawn(async move {
                                // Spawn new handler for each client connection
                                Server::handle_stream_notification(&handler, stream).await;
                            });
                        },

                        Err(e) => {
                            log::error!("Failed to accept connection: {e:?}");
                            continue;  // Keep listening instead of crashing
                        }
                    }
                },

                // Listen for state changes
                _ = arc_server.container_state_notify.notified() => {
                    // Check if state has been set to ShuttingDown
                    if ContainerState::from(arc_server.container_state.load(Ordering::SeqCst)) == ContainerState::ShuttingDown {
                        log::info!("Server {} graceful shutdown initiated", arc_server.address);
                        drop(listener);     // Extra explicit way to drop the connection
                        break;              // Breaks out of listening loop
                    }
                }
            }
        }

        log::info!("Waiting for ongoing tasks to complete...");
        while join_set.join_next().await.is_some() {} // Ensures all client handlers die before shutdown
        log::info!("Server {} shut down gracefully.", arc_server.address);

        // Server failure will bring the container down with it
        arc_server
            .container_state
            .store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
        arc_server.container_state_notify.notify_waiters();

        Ok(())
    }

    // ========================================================================
    //    STATIC: Deserialise incoming messages from clients
    // ========================================================================
    async fn handle_stream_notification(arc_server: &StdArc<Self>, mut stream: TcpStream) {
        let mut buf = vec![0u8; 65_536]; // This buffer can be increased/reduced depending on use case (currently 64KB) - is a vec to put it on heap
        let mut message_buf = Vec::new(); // Persistent buffer for accumulating message bytes - for handling partial messages

        loop {
            tokio::select! {
                // Attempt to read from stream (All results must be wrapped in Ok(),
                // other than inactivity err)
                read_result = stream.read(&mut buf) => {
                    match read_result {
                        Ok(0) => {
                            log::info!("Client disconnected gracefully.");
                            break;
                        }
                        Ok(n) => {
                            message_buf.extend_from_slice(&buf[..n]);

                            // Attempt to process all complete messages
                            while message_buf.len() >= 4 {
                                // Read the length prefix (first 4 bytes)
                                let len_bytes = &message_buf[..4];
                                let msg_len = u32::from_be_bytes(len_bytes.try_into().unwrap()) as usize;

                                // Do we have the full message yet?
                                if message_buf.len() < 4 + msg_len {
                                    break; // Wait for more data
                                }

                                // Extract message bytes and remove them from the buffer
                                let msg_bytes = message_buf[4..4 + msg_len].to_vec();
                                message_buf.drain(..4 + msg_len);

                                // Try to deserialize
                                match serialiser::deserialise_message(&msg_bytes) {
                                    Ok(msg) => {
                                        add_to_tx_with_retry(&arc_server.server_tx, &msg, "Server", "Main").await;
                                    }
                                    Err(e) => {
                                        log::warn!("Message (raw): {}", String::from_utf8_lossy(&msg_bytes));
                                        log::error!("Failed to deserialise message: {e:?}");
                                        continue;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to read from socket; err = {e:?}");
                            break;
                        }
                    }
                }
            }
        }
    }
}
