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
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc as StdArc, atomic::AtomicUsize};

// Package
use log::LevelFilter;
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{Notify, mpsc};
use tokio::task;
use tokio::time::{Duration, sleep};

// Local
use crate::containerisation::client_broker::ClientBroker;
use crate::containerisation::hyperion_container::HyperionContainer;
use crate::containerisation::traits::{
    ContainerIdentidy, HyperionContainerDirectiveMessage, Initialisable, LogLevel, Run,
};
use crate::logging::logging_service::initialise_logger;
use crate::network::network_topology::NetworkTopology;
use crate::network::server::Server;
use crate::utilities::load_config;

// A is the HyperionContainer Component template - must implement Initialisable and Run traits
// C is an StdArc instance of a populated config struct - specific to the component
// T is primary Component message type
pub async fn create<A, C, T>(
    config_path_str: &str,
    network_topology_path_str: &str,
    container_state: StdArc<AtomicUsize>,
    container_state_notify: StdArc<Notify>,
    main_rx: mpsc::Receiver<T>,
) -> HyperionContainer<A, T>
where
    A: Initialisable<ConfigType = C> + Run<Message = T> + Send + 'static + Sync + Clone + Debug,
    C: Debug + Send + 'static + DeserializeOwned + Sync + LogLevel + ContainerIdentidy,
    T: HyperionContainerDirectiveMessage
        + Debug
        + Send
        + 'static
        + DeserializeOwned
        + Sync
        + Clone
        + Serialize,
{
    // Read Component and network configs
    let config_path: PathBuf =
        fs::canonicalize(config_path_str).expect("Could not create path for component config");
    let component_config: StdArc<C> =
        load_config::load_config::<C>(config_path).expect("Component config could not be loaded");
    let network_topology_path = fs::canonicalize(network_topology_path_str)
        .expect("Could not create path for network config");
    let network_topology: StdArc<NetworkTopology> =
        load_config::load_config::<NetworkTopology>(network_topology_path)
            .expect("Component config could not be loaded");

    // Initialise logger
    initialise_logger(Some(
        LevelFilter::from_str(component_config.log_level()).unwrap_or(LevelFilter::Info),
    ))
    .unwrap();

    // Initialise console - temporary startup printout
    for (key, value) in component_config.container_identity().iter() {
        log::debug!("{key}: {value}");
    }

    // TODO: Improve startup messaging. Implement project boilerplate printout etc
    log::info!(
        "Building Hyperion Container for {}...",
        component_config
            .container_identity()
            .get("name")
            .unwrap_or(&"Unknown".to_string())
    );

    // Initialise component - Ensure the component can build without errors before starting comms
    let component_archetype = A::initialise(
        container_state.clone(),
        container_state_notify.clone(),
        component_config.clone(),
    );

    // Initialise and run Server
    let (server_tx, server_rx) = mpsc::channel::<T>(32);
    let arc_server: StdArc<Server<T>> = Server::new(
        network_topology.server_address.clone(),
        server_tx,
        container_state.clone(),
        container_state_notify.clone(),
    );
    task::spawn(async move {
        // No need to handle return as Server will set state to shutdown if it fails
        if let Err(e) = Server::run(arc_server).await {
            log::error!("Server encountered an error: {e:?}");
        }
    });

    // Allow time for server to stabilise
    sleep(Duration::from_secs(2)).await;

    // Initialise and run client broker
    let client_broker: ClientBroker<T> = ClientBroker::init(
        network_topology,
        container_state.clone(),
        container_state_notify.clone(),
    );

    // Allow time for client(s) to stabilise
    sleep(Duration::from_secs(2)).await;

    // Using previous elements, build HyperionContainer
    HyperionContainer::<A, T>::create(
        component_archetype,
        container_state,
        container_state_notify,
        client_broker,
        main_rx,
        server_rx,
    )
}
