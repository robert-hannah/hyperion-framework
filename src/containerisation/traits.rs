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
use std::sync::Arc as StdArc;
use std::sync::atomic::AtomicUsize;

// Package
use async_trait::async_trait;
use tokio::sync::Notify;
use tokio::sync::mpsc::{Receiver, Sender};

// Local
use crate::messages::client_broker_message::ClientBrokerMessage;
use crate::messages::container_directive::ContainerDirective;

// Traits
pub trait Initialisable {
    type ConfigType;
    fn initialise(
        container_state: StdArc<AtomicUsize>,
        container_state_notify: StdArc<Notify>,
        config: StdArc<Self::ConfigType>,
    ) -> Self;
}
// For example:
// Hyperion Network Containerisation - Initialise Component
// impl Initialisable for Component {
//     type ConfigType = Config;
//     fn initialise(container_state: StdArc<AtomicUsize>, container_state_notify: StdArc<Notify>, config: StdArc<Self::ConfigType>) -> Self {
//         // Will panic if there's a problem, which we want seeing we do this on startup
//         Component::new(container_state, container_state_notify, config)
//     }
// }

#[async_trait]
pub trait Run {
    // Instead of using async fn (which Rust bans for traits as return type size is unknown at compile time), run returns a future explicitly
    // The returned future must be Send, meaning it can be safely moved to another thread :) Using box so we can return many types of Futures
    // Future needs to be Pinned inside the box so its size can be known and be compatible with tokio runtime

    // OK, scratch all that, async_trait is a thing now. Not perfect and does introduce a small overhead, but it is a lot cleaner
    // Seeing we're only going to call this only once to start the component, it's not a big deal
    // Also there's no return type, hence the performance overhead -> can't optimise

    // Using self, instead of &mut self, as the clone must be consumed by Run to ensure a fresh state is kept in the container
    type Message;
    async fn run(
        self,
        comp_in_rx: Receiver<Self::Message>,
        comp_out_tx: Sender<ClientBrokerMessage<Self::Message>>,
    );
}
// For exmaple:
// Hyperion Network Containerisation - Run Component
// #[async_trait]
// impl Run for Component {
//     type Message = ContainerMessage;
//     async fn run(mut self, mut comp_in_rx: Receiver<Self::Message>, comp_out_tx: Sender<ClientBrokerMessage<Self::Message>>) {
//         log::debug!("{} has started successfully", self.config.container_id.name);
//         loop {
//             if self.component_state == ComponentState::Dead { break; }
//             tokio::select! {
//                 Some(message) = comp_in_rx.recv() => {
//                     log::trace!("{} received message: {:?}", self.config.container_id.name, message);
//                     if let Some(result) = self.process_incoming_message(message).await {
//                         let from_location = format!("{} main loop", self.config.container_id.name);
//                         let to_location = format!("{} Container", self.config.container_id.name);
//                         add_to_tx_with_retry(&comp_out_tx, &result, &from_location, &to_location).await;
//                     }
//                 }
//                 _ = self.container_state_notify.notified() => {
//                     // Check if container is shutting down
//                     if self.container_state.load(Ordering::SeqCst) == ContainerState::ShuttingDown as usize {
//                         self.component_state = ComponentState::Dormant;
//                         break;
//                     }
//                 }
//             }
//         }
//         log::info!("{} task has closed", self.config.container_id.name);
//     }
// }

pub trait HyperionContainerDirectiveMessage {
    fn get_container_directive_message(&self) -> Option<&ContainerDirective>;
}
//For exmaple:
// impl HyperionContainerDirectiveMessage for ContainerMessage {
//     // Gets ContainerDirective if is instance
//     fn get_container_directive_message(&self) -> Option<&ContainerDirective> {
//         if let ContainerMessage::ContainerDirectiveMsg(directive) = self {
//             Some(directive)
//         } else {
//             None
//         }
//     }
// }

pub trait ContainerIdentidy {
    fn container_identity(&self) -> HashMap<String, String>;
}
// For example:  (This is slightly memory inefficient because it uses hashmap but isn't too bad. Could use lazy static if being pedantic)
// impl ContainerIdentidy for Config {
//     fn container_identity(&self) -> HashMap<String, String> {
//         let mut identity = HashMap::new();
//         identity.insert("name".to_string(), self.container.name.clone());
//         identity.insert("version".to_string(), self.container.version.clone());
//         identity.insert("version_title".to_string(), self.container.version_title.clone());
//         identity.insert("software_collection".to_string(), self.container.software_collection.clone());
//         identity
//     }
// }

pub trait LogLevel {
    fn log_level(&self) -> &str;
}
// For example:
// impl LogLevel for Config {
//     fn log_level(&self) -> &str {
//         &self.logging.level
//     }
// }
