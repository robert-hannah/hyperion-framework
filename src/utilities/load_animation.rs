// // -------------------------------------------------------------------------------------------------
// // Hyperion Framework
// // https://github.com/Bazzz-1/hyperion-framework
// //
// // A lightweight Rust framework for building modular, component-based systems
// // with built-in TCP messaging and CLI control.
// //
// // Copyright 2025 Robert Hannah
// //
// // Licensed under the Apache License, Version 2.0 (the "License");
// // you may not use this file except in compliance with the License.
// // You may obtain a copy of the License at
// //
// //     http://www.apache.org/licenses/LICENSE-2.0
// //
// // Unless required by applicable law or agreed to in writing, software
// // distributed under the License is distributed on an "AS IS" BASIS,
// // WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// // See the License for the specific language governing permissions and
// // limitations under the License.
// // -------------------------------------------------------------------------------------------------
// 
// // Standard
// use std::io::{self, Write};
// use std::thread;
// use tokio::time::{Duration, sleep};
// 
// // Package
// use colored::ColoredString;
// 
// // Constants
// const SPINNER_CHARS: [&str; 4] = ["|", "/", "-", "\\"];
// 
// 
// pub async fn load_animation(print_string: &str, connection_result: &ColoredString) {
//     for _ in 0..10 {
//         for &ch in &SPINNER_CHARS {
//             print!("\r{print_string}{ch}"); // \r brings cursor back to start
//             io::stdout().flush().unwrap();
//             thread::sleep(Duration::from_millis(50));
//         }
//     }
// 
//     // Clear line and print result
//     print!("\r");
//     io::stdout().flush().unwrap();
//     println!(
//         "{}{}{}",
//         print_string,
//         find_print_gap(print_string),
//         connection_result
//     );
// 
//     sleep(Duration::from_millis(200)).await;
// }
// 
// fn find_print_gap(print_string: &str) -> String {
//     let gap: i8 = 30 + (40 - print_string.chars().count() as i8);
//     " ".repeat(gap as usize)
// }
