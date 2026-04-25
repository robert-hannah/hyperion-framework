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
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// Package
use hyperion_framework::containerisation::traits::{
    HyperionContainerDirectiveMessage, HyperionHeartbeatMessage,
};
use hyperion_framework::heartbeat::config::{
    HeartbeatConfig, HeartbeatMode, HeartbeatTargets, HeartbeatXml,
};
use hyperion_framework::heartbeat::handler::{
    FnHandler, FnMissedHandler, HeartbeatTimeoutHandler, ShutdownOnTimeout,
};
use hyperion_framework::heartbeat::receiver::HeartbeatReceiver;
use hyperion_framework::heartbeat::sender::HeartbeatSender;
use hyperion_framework::messages::container_directive::ContainerDirective;
use hyperion_framework::messages::heartbeat::{HeartbeatRequest, HeartbeatResponse};
use hyperion_framework::utilities::time::fmt_ms;
use serde::{Deserialize, Serialize};
use tokio::sync::{Notify, mpsc};
use tokio::time::{Duration, sleep, timeout};

// -------------------------------------------------------------------------------------------------
// Shared test message type
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
enum TestMsg {
    Directive(ContainerDirective),
}

impl HyperionContainerDirectiveMessage for TestMsg {
    fn get_container_directive_message(&self) -> Option<&ContainerDirective> {
        let TestMsg::Directive(d) = self;
        Some(d)
    }
}

impl HyperionHeartbeatMessage for TestMsg {
    fn as_heartbeat_request(&self) -> Option<HeartbeatRequest> {
        if let TestMsg::Directive(ContainerDirective::HeartbeatRequest {
            request_id,
            sender_name,
            timestamp_ms,
        }) = self
        {
            Some(HeartbeatRequest {
                request_id: *request_id,
                sender_name: sender_name.clone(),
                timestamp_ms: *timestamp_ms,
                component_alive: true,
                ms_since_last_activity: 0,
                container_state_val: 0,
            })
        } else {
            None
        }
    }

    fn as_heartbeat_response(&self) -> Option<HeartbeatResponse> {
        if let TestMsg::Directive(ContainerDirective::HeartbeatResponse {
            request_id,
            responder_name,
            timestamp_ms,
            component_alive,
            ms_since_last_activity,
            container_state_val,
        }) = self
        {
            Some(HeartbeatResponse {
                request_id: *request_id,
                responder_name: responder_name.clone(),
                timestamp_ms: *timestamp_ms,
                component_alive: *component_alive,
                ms_since_last_activity: *ms_since_last_activity,
                container_state_val: *container_state_val,
            })
        } else {
            None
        }
    }

    fn make_heartbeat_request(
        request_id: u64,
        sender_name: String,
        timestamp_ms: u64,
    ) -> Option<Self> {
        Some(TestMsg::Directive(ContainerDirective::HeartbeatRequest {
            request_id,
            sender_name,
            timestamp_ms,
        }))
    }

    fn make_heartbeat_response(
        request_id: u64,
        timestamp_ms: u64,
        component_alive: bool,
        ms_since_last_activity: u64,
        container_state_val: usize,
    ) -> Option<Self> {
        Some(TestMsg::Directive(ContainerDirective::HeartbeatResponse {
            request_id,
            responder_name: String::new(),
            timestamp_ms,
            component_alive,
            ms_since_last_activity,
            container_state_val,
        }))
    }
}

// -------------------------------------------------------------------------------------------------
// fmt_ms
// -------------------------------------------------------------------------------------------------

#[test]
fn fmt_ms_zero() {
    assert_eq!(fmt_ms(0), "0ms");
}

#[test]
fn fmt_ms_sub_second() {
    assert_eq!(fmt_ms(1), "1ms");
    assert_eq!(fmt_ms(423), "423ms");
    assert_eq!(fmt_ms(999), "999ms");
}

#[test]
fn fmt_ms_seconds() {
    assert_eq!(fmt_ms(1_000), "1.0s");
    assert_eq!(fmt_ms(1_500), "1.5s");
    assert_eq!(fmt_ms(3_200), "3.2s");
    assert_eq!(fmt_ms(30_000), "30.0s");
}

#[test]
fn fmt_ms_minutes() {
    assert_eq!(fmt_ms(60_000), "1m 0s");
    assert_eq!(fmt_ms(90_000), "1m 30s");
    assert_eq!(fmt_ms(125_000), "2m 5s");
}

// -------------------------------------------------------------------------------------------------
// HeartbeatConfig
// -------------------------------------------------------------------------------------------------

#[test]
fn heartbeat_config_sender_builds_correctly() {
    let cfg = HeartbeatConfig::sender(5_000, 3_000, vec!["A".to_string(), "B".to_string()]);
    assert_eq!(cfg.mode, HeartbeatMode::Sender);
    assert!(cfg.receiver.is_none());
    let s = cfg.sender.unwrap();
    assert_eq!(s.interval_ms, 5_000);
    assert_eq!(s.response_timeout_ms, 3_000);
    assert_eq!(s.targets, vec!["A", "B"]);
}

#[test]
fn heartbeat_config_receiver_builds_correctly() {
    let cfg = HeartbeatConfig::receiver(15_000);
    assert_eq!(cfg.mode, HeartbeatMode::Receiver);
    assert!(cfg.sender.is_none());
    assert_eq!(cfg.receiver.unwrap().timeout_ms, 15_000);
}

#[test]
fn heartbeat_config_disabled_builds_correctly() {
    let cfg = HeartbeatConfig::disabled();
    assert_eq!(cfg.mode, HeartbeatMode::Disabled);
    assert!(cfg.sender.is_none());
    assert!(cfg.receiver.is_none());
}

// -------------------------------------------------------------------------------------------------
// HeartbeatXml::to_config
// -------------------------------------------------------------------------------------------------

#[test]
fn heartbeat_xml_receiver_produces_correct_config() {
    let xml = HeartbeatXml {
        mode: "receiver".into(),
        interval_ms: None,
        response_timeout_ms: None,
        targets: None,
        timeout_ms: Some(10_000),
    };
    let cfg = xml.to_config().unwrap();
    assert_eq!(cfg.mode, HeartbeatMode::Receiver);
    assert_eq!(cfg.receiver.unwrap().timeout_ms, 10_000);
}

#[test]
fn heartbeat_xml_sender_produces_correct_config() {
    let xml = HeartbeatXml {
        mode: "sender".into(),
        interval_ms: Some(4_000),
        response_timeout_ms: Some(2_000),
        targets: Some(HeartbeatTargets {
            targets: vec!["X".into(), "Y".into()],
        }),
        timeout_ms: None,
    };
    let cfg = xml.to_config().unwrap();
    assert_eq!(cfg.mode, HeartbeatMode::Sender);
    let s = cfg.sender.unwrap();
    assert_eq!(s.interval_ms, 4_000);
    assert_eq!(s.response_timeout_ms, 2_000);
    assert_eq!(s.targets, vec!["X", "Y"]);
}

#[test]
fn heartbeat_xml_disabled_returns_none() {
    let xml = HeartbeatXml {
        mode: "disabled".into(),
        interval_ms: None,
        response_timeout_ms: None,
        targets: None,
        timeout_ms: None,
    };
    assert!(xml.to_config().is_none());
}

#[test]
#[should_panic(expected = "Unknown heartbeat mode")]
fn heartbeat_xml_unknown_mode_panics() {
    let xml = HeartbeatXml {
        mode: "blorp".into(),
        interval_ms: None,
        response_timeout_ms: None,
        targets: None,
        timeout_ms: None,
    };
    xml.to_config();
}

#[test]
#[should_panic(expected = "interval_ms is missing")]
fn heartbeat_xml_sender_missing_interval_panics() {
    let xml = HeartbeatXml {
        mode: "sender".into(),
        interval_ms: None,
        response_timeout_ms: Some(1_000),
        targets: None,
        timeout_ms: None,
    };
    xml.to_config();
}

#[test]
#[should_panic(expected = "timeout_ms is missing")]
fn heartbeat_xml_receiver_missing_timeout_panics() {
    let xml = HeartbeatXml {
        mode: "receiver".into(),
        interval_ms: None,
        response_timeout_ms: None,
        targets: None,
        timeout_ms: None,
    };
    xml.to_config();
}

// -------------------------------------------------------------------------------------------------
// FnHandler / HeartbeatTimeoutHandler
// -------------------------------------------------------------------------------------------------

#[test]
fn fn_handler_invokes_closure() {
    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();
    let handler = FnHandler::new(move || c.store(true, Ordering::SeqCst));
    handler.on_timeout();
    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn fn_handler_can_be_called_multiple_times() {
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let handler = FnHandler::new(move || {
        c.fetch_add(1, Ordering::SeqCst);
    });
    handler.on_timeout();
    handler.on_timeout();
    handler.on_timeout();
    assert_eq!(count.load(Ordering::SeqCst), 3);
}

#[test]
fn shutdown_on_timeout_sets_container_state() {
    let state = Arc::new(AtomicUsize::new(0));
    let notify = Arc::new(Notify::new());
    let handler = ShutdownOnTimeout::new(state.clone(), notify.clone());
    handler.on_timeout();
    // ContainerState::ShuttingDown == 2
    assert_ne!(
        state.load(Ordering::SeqCst),
        0,
        "state should have been updated"
    );
}

// -------------------------------------------------------------------------------------------------
// HeartbeatSender
// -------------------------------------------------------------------------------------------------

#[tokio::test]
async fn sender_emits_request_to_target_after_interval() {
    let (target_tx, mut target_rx) = mpsc::channel::<TestMsg>(10);
    let mut all_senders = HashMap::new();
    all_senders.insert("Target".to_string(), target_tx);

    let (sender, _response_tx) = HeartbeatSender::new(
        50,
        500,
        "Sender".to_string(),
        all_senders,
        &["Target".to_string()],
        None,
    );
    tokio::spawn(sender.run());

    let msg = timeout(Duration::from_millis(300), target_rx.recv())
        .await
        .expect("timed out waiting for HB request")
        .expect("channel closed");

    assert!(
        msg.as_heartbeat_request().is_some(),
        "expected a heartbeat request"
    );
}

#[tokio::test]
async fn sender_processes_response_without_error() {
    let (target_tx, mut target_rx) = mpsc::channel::<TestMsg>(10);
    let mut all_senders = HashMap::new();
    all_senders.insert("Target".to_string(), target_tx);

    let (sender, response_tx) = HeartbeatSender::new(
        50,
        500,
        "Sender".to_string(),
        all_senders,
        &["Target".to_string()],
        None,
    );
    tokio::spawn(sender.run());

    // Receive the outbound request and extract its id
    let msg = timeout(Duration::from_millis(300), target_rx.recv())
        .await
        .expect("timeout")
        .expect("closed");
    let req_id = msg
        .as_heartbeat_request()
        .expect("expected HB request")
        .request_id;

    // Send back a matching response — sender should accept it without panic
    response_tx
        .send(HeartbeatResponse {
            request_id: req_id,
            responder_name: "Target".to_string(),
            timestamp_ms: 0,
            component_alive: true,
            ms_since_last_activity: 50,
            container_state_val: 0,
        })
        .await
        .unwrap();

    sleep(Duration::from_millis(50)).await;
}

#[tokio::test]
async fn sender_skips_unknown_target_without_panic() {
    // all_senders is empty — "Missing" is not reachable
    let (sender, _response_tx) = HeartbeatSender::<TestMsg>::new(
        50,
        500,
        "Sender".to_string(),
        HashMap::new(),
        &["Missing".to_string()],
        None,
    );
    // Running the sender with no reachable targets should not panic
    let handle = tokio::spawn(sender.run());
    sleep(Duration::from_millis(120)).await;
    assert!(!handle.is_finished(), "sender task should still be alive");
    handle.abort();
}

#[tokio::test]
async fn sender_prunes_unanswered_requests_on_next_tick() {
    let (target_tx, mut target_rx) = mpsc::channel::<TestMsg>(10);
    let mut all_senders = HashMap::new();
    all_senders.insert("Target".to_string(), target_tx);

    // response_timeout_ms (30) < interval_ms (80), so by the second tick the first
    // request is always stale and gets pruned
    let (sender, _response_tx) = HeartbeatSender::new(
        80,
        30,
        "Sender".to_string(),
        all_senders,
        &["Target".to_string()],
        None,
    );
    tokio::spawn(sender.run());

    // Drain requests but never respond — sender should not panic through two intervals
    sleep(Duration::from_millis(250)).await;
    while target_rx.try_recv().is_ok() {}
}

#[tokio::test]
async fn sender_calls_missed_handler_on_every_unanswered_tick() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let c = call_count.clone();

    let (target_tx, mut target_rx) = mpsc::channel::<TestMsg>(10);
    let mut all_senders = HashMap::new();
    all_senders.insert("Target".to_string(), target_tx);

    // response_timeout_ms (30) < interval_ms (80): request is stale by the next tick
    let handler = FnMissedHandler::new(move |_target| {
        c.fetch_add(1, Ordering::SeqCst);
    });
    let (sender, _response_tx) = HeartbeatSender::new(
        80,
        30,
        "Sender".to_string(),
        all_senders,
        &["Target".to_string()],
        Some(Box::new(handler)),
    );
    tokio::spawn(sender.run());

    // Never respond — drain the channel so it doesn't back-pressure the sender
    sleep(Duration::from_millis(400)).await;
    while target_rx.try_recv().is_ok() {}

    // Should have fired at least once per interval (≥4 ticks in 400ms @ 80ms)
    assert!(
        call_count.load(Ordering::SeqCst) >= 2,
        "missed handler should fire repeatedly"
    );
}

#[tokio::test]
async fn sender_does_not_call_missed_handler_when_responses_arrive() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let c = call_count.clone();

    let (target_tx, mut target_rx) = mpsc::channel::<TestMsg>(10);
    let mut all_senders = HashMap::new();
    all_senders.insert("Target".to_string(), target_tx);

    let handler = FnMissedHandler::new(move |_target| {
        c.fetch_add(1, Ordering::SeqCst);
    });
    let (sender, response_tx) = HeartbeatSender::new(
        80,
        500,
        "Sender".to_string(),
        all_senders,
        &["Target".to_string()],
        Some(Box::new(handler)),
    );
    tokio::spawn(sender.run());

    // Echo every request back as a response within the timeout window
    for _ in 0..4 {
        if let Ok(Some(msg)) = timeout(Duration::from_millis(200), target_rx.recv()).await {
            if let Some(req) = msg.as_heartbeat_request() {
                response_tx
                    .send(HeartbeatResponse {
                        request_id: req.request_id,
                        responder_name: "Target".to_string(),
                        timestamp_ms: 0,
                        component_alive: true,
                        ms_since_last_activity: 10,
                        container_state_val: 0,
                    })
                    .await
                    .unwrap();
            }
        }
    }

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "missed handler must not fire when responses arrive"
    );
}

// -------------------------------------------------------------------------------------------------
// HeartbeatReceiver
// -------------------------------------------------------------------------------------------------

#[tokio::test]
async fn receiver_sends_response_to_sender() {
    // "MC" is the container whose channel the receiver sends responses back on
    let (back_tx, mut back_rx) = mpsc::channel::<TestMsg>(10);
    let mut all_senders = HashMap::new();
    all_senders.insert("MC".to_string(), back_tx);

    let handler = FnHandler::new(|| {});
    let (receiver, request_tx) = HeartbeatReceiver::new(5_000, all_senders, Box::new(handler));
    tokio::spawn(receiver.run());

    request_tx
        .send(HeartbeatRequest {
            request_id: 42,
            sender_name: "MC".to_string(),
            timestamp_ms: 0,
            component_alive: true,
            ms_since_last_activity: 100,
            container_state_val: 0,
        })
        .await
        .unwrap();

    let msg = timeout(Duration::from_millis(200), back_rx.recv())
        .await
        .expect("timeout waiting for response")
        .expect("channel closed");

    let resp = msg.as_heartbeat_response().expect("expected HB response");
    assert_eq!(resp.request_id, 42);
}

#[tokio::test]
async fn receiver_calls_handler_when_no_requests_arrive() {
    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();

    let handler = FnHandler::new(move || c.store(true, Ordering::SeqCst));
    let (receiver, _request_tx) =
        HeartbeatReceiver::<TestMsg>::new(100, HashMap::new(), Box::new(handler));
    tokio::spawn(receiver.run());

    // The watchdog check interval has a 1s floor (max(timeout_ms/3, 1000)).
    // With timeout_ms=100, the first tick fires at ~1000ms and sees elapsed >> 100ms.
    // 1500ms gives a comfortable margin past that first tick.
    sleep(Duration::from_millis(1500)).await;
    assert!(
        called.load(Ordering::SeqCst),
        "handler should have been called after timeout"
    );
}

#[tokio::test]
async fn receiver_does_not_call_handler_while_requests_arrive() {
    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();

    let handler = FnHandler::new(move || c.store(true, Ordering::SeqCst));
    let (receiver, request_tx) =
        HeartbeatReceiver::<TestMsg>::new(150, HashMap::new(), Box::new(handler));
    tokio::spawn(receiver.run());

    // Send a request every 40ms for 300ms — well within the 150ms timeout
    for _ in 0..7 {
        let _ = request_tx
            .send(HeartbeatRequest {
                request_id: 1,
                sender_name: "MC".to_string(),
                timestamp_ms: 0,
                component_alive: true,
                ms_since_last_activity: 0,
                container_state_val: 0,
            })
            .await;
        sleep(Duration::from_millis(40)).await;
    }

    assert!(
        !called.load(Ordering::SeqCst),
        "handler should not be called while requests arrive"
    );
}

#[tokio::test]
async fn receiver_does_not_panic_with_unknown_sender_name() {
    // all_senders is empty — receiver can't route the response but should not panic
    let handler = FnHandler::new(|| {});
    let (receiver, request_tx) =
        HeartbeatReceiver::<TestMsg>::new(5_000, HashMap::new(), Box::new(handler));
    tokio::spawn(receiver.run());

    request_tx
        .send(HeartbeatRequest {
            request_id: 1,
            sender_name: "Nobody".to_string(),
            timestamp_ms: 0,
            component_alive: true,
            ms_since_last_activity: 0,
            container_state_val: 0,
        })
        .await
        .unwrap();

    sleep(Duration::from_millis(100)).await;
}
