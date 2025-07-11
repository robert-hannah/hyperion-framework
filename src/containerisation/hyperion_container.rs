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
use std::fmt::Debug;
use std::sync::Arc as StdArc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Package
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{Notify, mpsc};
use tokio::task;
use tokio::time::{Duration, sleep};

// Local
use crate::containerisation::client_broker::ClientBroker;
use crate::containerisation::container_state::ContainerState;
use crate::containerisation::traits::{HyperionContainerDirectiveMessage, Run};
use crate::messages::client_broker_message::ClientBrokerMessage;
use crate::messages::container_directive::ContainerDirective;
use crate::utilities::tx_sender::add_to_tx_with_retry;

// A is the HyperionContainer Component template - must implement Initialisable and Run traits
// T is the primary message type
#[allow(dead_code)]
pub struct HyperionContainer<A, T> {
    component_archetype: A, // Can be used to restart the component task
    component_handle: task::JoinHandle<()>,
    container_state: StdArc<AtomicUsize>,
    container_state_notify: StdArc<Notify>,
    client_broker: ClientBroker<T>,
    component_in_tx: mpsc::Sender<T>,
    component_out_rx: mpsc::Receiver<ClientBrokerMessage<T>>,
    main_rx: mpsc::Receiver<T>,
    server_rx: mpsc::Receiver<T>,
}

// TODO: Implementations need separated out as not all implement all template types
impl<A, T> HyperionContainer<A, T>
where
    A: Run<Message = T> + Send + 'static + Sync + Clone + Debug,
    T: HyperionContainerDirectiveMessage
        + Debug
        + Send
        + 'static
        + DeserializeOwned
        + Sync
        + Clone
        + Serialize,
{
    pub fn create(
        component_archetype: A,
        container_state: StdArc<AtomicUsize>,
        container_state_notify: StdArc<Notify>,
        client_broker: ClientBroker<T>,
        main_rx: mpsc::Receiver<T>,
        server_rx: mpsc::Receiver<T>,
    ) -> Self {
        log::info!("Starting Hyperion Container...");
        let (component_in_tx, component_in_rx) = mpsc::channel::<T>(32);
        let (component_out_tx, component_out_rx) = mpsc::channel::<ClientBrokerMessage<T>>(32);
        let component_handle: task::JoinHandle<()> = HyperionContainer::start_component(
            component_archetype.clone(),
            component_in_rx,
            component_out_tx,
        );
        Self {
            component_archetype,
            component_handle,
            container_state,
            container_state_notify,
            client_broker,
            component_in_tx,
            component_out_rx,
            main_rx,
            server_rx,
        }
    }

    fn start_component(
        component_archetype: A,
        component_in_rx: mpsc::Receiver<T>,
        component_out_tx: mpsc::Sender<ClientBrokerMessage<T>>,
    ) -> task::JoinHandle<()> {
        let component = component_archetype.clone();
        let component_task: task::JoinHandle<()> = {
            task::spawn(async move {
                component.run(component_in_rx, component_out_tx).await;
            })
        };
        component_task
    }

    pub async fn run(&mut self) {
        // HyperionContainer main loop
        log::info!("Hyperion Container is running!");
        loop {
            // Check if Container is dying
            let state = self.container_state.load(Ordering::SeqCst);
            if state == ContainerState::ShuttingDown as usize
                || state == ContainerState::DeadComponent as usize
            {
                log::info!("Container is shutting down...");
                self.container_state
                    .store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
                self.container_state_notify.notify_waiters();

                // Allow time for comms to stop etc before stoppping main.rs
                sleep(Duration::from_secs(3)).await;
                self.container_state
                    .store(ContainerState::Closed as usize, Ordering::SeqCst);
                self.container_state_notify.notify_waiters();
                break;
            }

            // Check Component task handle
            if self.component_handle.is_finished() {
                log::warn!("Component task has finished unexpectedly.");
                self.container_state
                    .store(ContainerState::DeadComponent as usize, Ordering::SeqCst);
                self.container_state_notify.notify_waiters();
            }

            // Process incoming and outgoing messages
            tokio::select! {
                Some(message) = self.main_rx.recv() => {                // Messages from command line
                    log::trace!("Container received message form console: {message:?}");
                    self.process_incoming_message(message).await;
                }
                Some(message) = self.server_rx.recv() => {              // Messages from Server
                    log::trace!("Container received message form server: {message:?}");
                    self.process_incoming_message(message).await;
                }
                Some(message) = self.component_out_rx.recv() => {       // Messages from Component
                    log::trace!("Container received message form Component: {message:?}");
                    self.client_broker.handle_message(message).await;
                }
            }
        }
    }

    async fn process_incoming_message(&mut self, message: T)
    where
        T: HyperionContainerDirectiveMessage + Debug + Clone + Send + 'static,
    {
        if let Some(container_directive) = message.get_container_directive_message() {
            match container_directive {
                ContainerDirective::Shutdown => {
                    log::info!("Container received shutdown directive");
                    // Set shutdown state
                    self.container_state
                        .store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
                    self.container_state_notify.notify_waiters();
                }
                ContainerDirective::SystemShutdown => {
                    log::info!("Container received system shutdown directive");
                    // Forward shutdown message
                    self.client_broker.forward_shutdown(message.clone()).await;
                    // Set shutdown state
                    self.container_state
                        .store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
                    self.container_state_notify.notify_waiters();
                    // Wait for clients to finish
                    self.client_broker.shutdown().await;
                }
                _ => {
                    log::warn!("Container received unmapped directive: {container_directive:?}");
                }
            }
        } else {
            // Send to component without inspection (ComponentDirective or non-generic message)
            log::trace!("Forwarding non-framework message: {message:?}");
            add_to_tx_with_retry(
                &self.component_in_tx,
                &message,
                "Container main loop",
                "Component main loop",
            )
            .await;
        }
    }
}
