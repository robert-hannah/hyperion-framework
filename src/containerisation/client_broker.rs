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
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc as StdArc, atomic::AtomicUsize};

// Package
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};

// Local
use crate::messages::client_broker_message::ClientBrokerMessage;
use crate::network::client::Client;
use crate::network::network_topology::{Connection, NetworkTopology};
use crate::utilities::tx_sender::add_to_tx_with_retry;

pub struct ClientBroker<T> {
    messaging_active: bool,
    // network_topology:           StdArc<NetworkTopology>,        // TODO: Use this to restart connections?
    client_senders: HashMap<String, mpsc::Sender<T>>, // TODO: Make this not a hash map
    client_threads: JoinSet<()>,
}

// TODO: Check client state for restart when needed
// TODO: Actual broker part

impl<T> ClientBroker<T>
where
    T: Debug + Send + 'static + DeserializeOwned + Sync + Clone + Serialize,
{
    pub fn init(
        network_topology: StdArc<NetworkTopology>,
        container_state: StdArc<AtomicUsize>,
        container_state_notify: StdArc<Notify>,
    ) -> Self {
        // Gather connections
        // This must be cloned as topolgy currently doesn't live that long
        let connections: Vec<Connection> = network_topology
            .client_connections
            .client_connection_vec
            .clone();

        let mut client_senders = HashMap::new();
        let mut client_threads = JoinSet::new();

        // Create clients from connections
        for conn in connections {
            let (client_tx, client_rx) = mpsc::channel::<T>(32);
            client_senders.insert(conn.name.clone(), client_tx);

            // Create and spawn the client on a new thread
            let container_state_clone = container_state.clone();
            let container_state_notify_clone = container_state_notify.clone();
            let client = Client::new(
                conn.name.clone(),
                conn.address.clone(),
                client_rx,
                container_state_clone,
                container_state_notify_clone,
                5,
            );

            client_threads.spawn(async move {
                if let Err(e) = client.run().await {
                    log::error!("Client {} encountered an error: {e:?}", conn.name);
                }
            });
        }

        Self {
            messaging_active: true,
            client_senders,
            client_threads,
        }
    }

    pub async fn handle_message(&self, message: ClientBrokerMessage<T>) {
        if self.messaging_active {
            for target_client in &message.target_clients {
                if let Some(sender) = self.client_senders.get(target_client) {
                    add_to_tx_with_retry(sender, &message.message, "ClientBroker", target_client)
                        .await;
                } else {
                    log::error!("Unknown TargetClient: {target_client}");
                }
            }
        } else {
            log::trace!(
                "Message to {} blocked by inactive ClientBroker",
                message.target_clients.concat()
            );
        }
    }

    pub async fn forward_shutdown(&mut self, message: T) {
        // Send shutdown command across all clients
        for (name, sender) in &self.client_senders {
            if let Err(e) = sender.send(message.clone()).await {
                log::error!("Failed to send shutdown message to {name}: {e:?}");
            }
        }
        sleep(Duration::from_millis(500)).await;
        self.messaging_active = false;
    }

    pub async fn shutdown(&mut self) {
        // Wait for clients to finish
        // ContainerState must be set to shutdown before this is called
        while self.client_threads.join_next().await.is_some() {}
        self.client_senders.clear(); // Drop all senders explicitly
        log::info!("ClientBroker has closed all Clients");
    }
}

impl<T> Drop for ClientBroker<T> {
    fn drop(&mut self) {
        log::info!("ClientBroker has been closed");
    }
}
