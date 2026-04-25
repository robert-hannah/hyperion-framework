// -------------------------------------------------------------------------------------------------
// Hyperion Framework
// https://github.com/robert-hannah/hyperion-framework
//
// Copyright 2025 Robert Hannah — Apache-2.0
// -------------------------------------------------------------------------------------------------

// Standard
use std::fmt::Debug;
use std::sync::Arc as StdArc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

// Package
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{Notify, mpsc};
use tokio::task;
use tokio::time::{Duration, sleep};

// Local
use crate::containerisation::client_broker::ClientBroker;
use crate::containerisation::container_state::ContainerState;
use crate::containerisation::traits::{HyperionContainerDirectiveMessage, HyperionHeartbeatMessage, Run};
use crate::heartbeat::config::{HeartbeatConfig, HeartbeatMode};
use crate::heartbeat::handler::{HeartbeatMissedHandler, HeartbeatTimeoutHandler};
use crate::heartbeat::receiver::HeartbeatReceiver;
use crate::heartbeat::sender::HeartbeatSender;
use crate::messages::client_broker_message::ClientBrokerMessage;
use crate::messages::container_directive::ContainerDirective;
use crate::messages::heartbeat::{HeartbeatRequest, HeartbeatResponse};
use crate::utilities::time::current_epoch_ms;
use crate::utilities::tx_sender::add_to_tx_with_retry;

#[allow(dead_code)]
pub struct HyperionContainer<T> {
    component_handle: task::JoinHandle<()>,
    container_state: StdArc<AtomicUsize>,
    container_state_notify: StdArc<Notify>,
    client_broker: ClientBroker<T>,
    component_in_tx: mpsc::Sender<T>,
    component_out_rx: mpsc::Receiver<ClientBrokerMessage<T>>,
    main_rx: mpsc::Receiver<T>,
    server_rx: mpsc::Receiver<T>,
    last_activity_ms: StdArc<AtomicU64>,
    heartbeat_request_tx: Option<mpsc::Sender<HeartbeatRequest>>,
    heartbeat_response_tx: Option<mpsc::Sender<HeartbeatResponse>>,
}

impl<T> HyperionContainer<T>
where
    T: HyperionContainerDirectiveMessage
        + HyperionHeartbeatMessage
        + Debug
        + Send
        + 'static
        + DeserializeOwned
        + Sync
        + Clone
        + Serialize,
{
    pub fn create<A>(
        component_archetype: A,
        container_state: StdArc<AtomicUsize>,
        container_state_notify: StdArc<Notify>,
        client_broker: ClientBroker<T>,
        main_rx: mpsc::Receiver<T>,
        server_rx: mpsc::Receiver<T>,
        heartbeat_config: Option<HeartbeatConfig>,
        container_name: String,
        timeout_handler: Option<Box<dyn HeartbeatTimeoutHandler>>,
        missed_handler: Option<Box<dyn HeartbeatMissedHandler>>,
    ) -> Self
    where
        A: Run<Message = T> + Send + 'static + Sync + Debug,
    {
        log::info!("Starting Hyperion Container...");
        let (component_in_tx, component_in_rx) = mpsc::channel::<T>(32);
        let (component_out_tx, component_out_rx) = mpsc::channel::<ClientBrokerMessage<T>>(32);
        let component_handle = HyperionContainer::start_component(
            component_archetype,
            component_in_rx,
            component_out_tx,
        );

        let last_activity_ms = StdArc::new(AtomicU64::new(current_epoch_ms()));
        let all_senders = client_broker.clone_all_senders();

        let (heartbeat_request_tx, heartbeat_response_tx) = Self::spawn_heartbeat_tasks(
            heartbeat_config,
            container_name,
            all_senders,
            timeout_handler,
            missed_handler,
        );

        Self {
            component_handle,
            container_state,
            container_state_notify,
            client_broker,
            component_in_tx,
            component_out_rx,
            main_rx,
            server_rx,
            last_activity_ms,
            heartbeat_request_tx,
            heartbeat_response_tx,
        }
    }

    fn start_component<A>(
        component_archetype: A,
        component_in_rx: mpsc::Receiver<T>,
        component_out_tx: mpsc::Sender<ClientBrokerMessage<T>>,
    ) -> task::JoinHandle<()>
    where
        A: Run<Message = T> + Send + 'static + Sync + Debug,
    {
        task::spawn(async move {
            component_archetype.run(component_in_rx, component_out_tx).await;
        })
    }

    fn spawn_heartbeat_tasks(
        heartbeat_config: Option<HeartbeatConfig>,
        container_name: String,
        all_senders: std::collections::HashMap<String, mpsc::Sender<T>>,
        timeout_handler: Option<Box<dyn HeartbeatTimeoutHandler>>,
        missed_handler: Option<Box<dyn HeartbeatMissedHandler>>,
    ) -> (Option<mpsc::Sender<HeartbeatRequest>>, Option<mpsc::Sender<HeartbeatResponse>>) {
        let config = match heartbeat_config {
            Some(c) if c.mode != HeartbeatMode::Disabled => c,
            _ => return (None, None),
        };

        match config.mode {
            HeartbeatMode::Sender => {
                let sender_cfg = match config.sender {
                    Some(c) => c,
                    None => {
                        log::error!("HeartbeatMode::Sender set but no sender config provided");
                        return (None, None);
                    }
                };
                let (sender_task, response_tx) = HeartbeatSender::new(
                    sender_cfg.interval_ms,
                    sender_cfg.response_timeout_ms,
                    container_name,
                    all_senders,
                    &sender_cfg.targets,
                    missed_handler,
                );
                task::spawn(async move { sender_task.run().await });
                (None, Some(response_tx))
            }
            HeartbeatMode::Receiver => {
                let receiver_cfg = match config.receiver {
                    Some(c) => c,
                    None => {
                        log::error!("HeartbeatMode::Receiver set but no receiver config provided");
                        return (None, None);
                    }
                };
                let handler = match timeout_handler {
                    Some(h) => h,
                    None => {
                        log::error!("HeartbeatMode::Receiver set but no timeout handler provided");
                        return (None, None);
                    }
                };
                let (receiver_task, request_tx) = HeartbeatReceiver::new(
                    receiver_cfg.timeout_ms,
                    all_senders,
                    handler,
                );
                task::spawn(async move { receiver_task.run().await });
                (Some(request_tx), None)
            }
            HeartbeatMode::Disabled => (None, None),
        }
    }

    pub async fn run(&mut self) {
        log::info!("Hyperion Container is running!");
        loop {
            let state = self.container_state.load(Ordering::SeqCst);
            if state == ContainerState::ShuttingDown as usize
                || state == ContainerState::DeadComponent as usize
            {
                log::info!("Container is shutting down...");
                self.container_state
                    .store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
                self.container_state_notify.notify_waiters();

                sleep(Duration::from_secs(3)).await;
                self.container_state
                    .store(ContainerState::Closed as usize, Ordering::SeqCst);
                self.container_state_notify.notify_waiters();
                break;
            }

            if self.component_handle.is_finished() {
                log::warn!("Component task has finished unexpectedly.");
                self.container_state
                    .store(ContainerState::DeadComponent as usize, Ordering::SeqCst);
                self.container_state_notify.notify_waiters();
            }

            tokio::select! {
                Some(message) = self.main_rx.recv() => {
                    log::trace!("Container received message from console: {message:?}");
                    self.process_incoming_message(message).await;
                }
                Some(message) = self.server_rx.recv() => {
                    log::trace!("Container received message from server: {message:?}");
                    self.process_incoming_message(message).await;
                }
                Some(message) = self.component_out_rx.recv() => {
                    log::trace!("Container received message from Component: {message:?}");
                    self.last_activity_ms.store(current_epoch_ms(), Ordering::SeqCst);
                    self.client_broker.handle_message(message).await;
                }
            }
        }
    }

    async fn process_incoming_message(&mut self, message: T) {
        // Heartbeat messages are handled entirely at container level — never forwarded to component.
        if let Some(req) = message.as_heartbeat_request() {
            if let Some(tx) = &self.heartbeat_request_tx {
                let enriched = HeartbeatRequest {
                    request_id: req.request_id,
                    sender_name: req.sender_name,
                    timestamp_ms: req.timestamp_ms,
                    component_alive: !self.component_handle.is_finished(),
                    ms_since_last_activity: current_epoch_ms()
                        .saturating_sub(self.last_activity_ms.load(Ordering::SeqCst)),
                    container_state_val: self.container_state.load(Ordering::SeqCst),
                };
                let _ = tx.try_send(enriched);
            }
            return;
        }
        if let Some(resp) = message.as_heartbeat_response() {
            if let Some(tx) = &self.heartbeat_response_tx {
                let _ = tx.try_send(resp);
            }
            return;
        }

        // Standard container directives
        if let Some(container_directive) = message.get_container_directive_message() {
            match container_directive {
                ContainerDirective::Shutdown => {
                    log::info!("Container received shutdown directive");
                    self.container_state
                        .store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
                    self.container_state_notify.notify_waiters();
                }
                ContainerDirective::SystemShutdown => {
                    log::info!("Container received system shutdown directive");
                    self.client_broker.forward_shutdown(message.clone()).await;
                    self.container_state
                        .store(ContainerState::ShuttingDown as usize, Ordering::SeqCst);
                    self.container_state_notify.notify_waiters();
                    self.client_broker.shutdown().await;
                }
                _ => {
                    log::warn!("Container received unmapped directive: {container_directive:?}");
                }
            }
        } else {
            log::trace!("Forwarding non-framework message to component: {message:?}");
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
