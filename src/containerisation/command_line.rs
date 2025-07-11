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


// TODO: This is WIP, stopped due to the use of ContainerMessage which can't be used within Hyperion

// // Package
// use crate::messages::container_directive::ContainerDirective;
// use crate::messages::component_directive::ComponentDirective;
// use tokio::io::{AsyncBufReadExt, BufReader, stdin};


// pub async fn run_command_line_interface() {
//     let mut reader = BufReader::new(stdin()).lines(); // Buffered async reader
//     log::info!("Enter a command or press h for help.");
//     loop {
//         tokio::select! {
//             Ok(Some(command)) = reader.next_line() => {  // Non-blocking input reading
//                 let command = command.trim();
//                 match command {
//                     "h" => {
//                         println!("Commands:");
//                         println!("start         - Component state set to active");
//                         println!("suspend       - Component state set to dormant");
//                         println!("c .           - Delete and respawn all connections");
//                         println!("s             - Graceful container shutdown");
//                         println!("s .           - Graceful container network shutdown");
//                     }
//                     "start" => {
//                         add_to_tx_with_retry(&main_tx, &ContainerMessage::ComponentDirectiveMsg(ComponentDirective::SetToActive), "Command line", "Container main").await;
//                     }
//                     "suspend" => {
//                         add_to_tx_with_retry(&main_tx, &ContainerMessage::ComponentDirectiveMsg(ComponentDirective::SetToDormant), "Command line", "Container main").await;
//                     }
//                     "c ." => {
//                         add_to_tx_with_retry(&main_tx, &ContainerMessage::ContainerDirectiveMsg(ContainerDirective::RetryAllConnections), "Command line", "Container main").await;
//                     }
//                     "s" => {
//                         add_to_tx_with_retry(&main_tx, &ContainerMessage::ContainerDirectiveMsg(ContainerDirective::Shutdown), "Command line", "Container main").await;
//                     }
//                     _ => println!("Unknown command: {}", command),
//                 }
//             }
//             _ = container_state_notify.notified() => {
//                 if container_state.load(Ordering::SeqCst) == ContainerState::Closed as usize {
//                     break;
//                 }
//             }
//         }
//     }
// }