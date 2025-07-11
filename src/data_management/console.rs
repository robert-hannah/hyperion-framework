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

// // Standard
// use std::sync::{Arc as StdArc, atomic::{AtomicUsize, Ordering}};
// use std::{
//     error::Error,
//     fs::OpenOptions,
//     io::{self, Write},
// };

// // Package
// use tokio::time::{sleep, Duration};
// use tokio::sync::{mpsc, Notify};
// use crossterm::{
//     event::{self, KeyCode, KeyEvent},
//     execute,
//     terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
// };
// use ratatui::{
//     backend::CrosstermBackend,
//     layout::{Constraint, Direction, Layout},
//     style::{Color, Modifier, Style},
//     text::Span,
//     widgets::{Block, Borders, List, ListItem, Paragraph},
//     Terminal,
// };
// use chrono::Utc;

// // Local
// use crate::containerisation::container_state::ContainerState;
// use crate::network::network_topology::NetworkTopology;

// pub struct Console {
//     container_name: String,
//     server_address: String,
//     client_names: Vec<String>,
//     container_state: StdArc<AtomicUsize>,
//     container_state_notify: StdArc<Notify>,
//     console_rx: mpsc::Receiver<String>,
//     log_file_name: String
// }

// fn log_to_file(log: &str) -> io::Result<()> {
//     let mut file = OpenOptions::new()
//         .create(true)
//         .append(true)
//         .open("log")?;
//     writeln!(file, "{}", log)?;
//     Ok(())
// }

// impl Console {
//     pub fn new(netowrk_topology: StdArc<NetworkTopology>, container_state: StdArc<AtomicUsize>, container_state_notify: StdArc<Notify>, console_rx: mpsc::Receiver<String>) -> Self {
//         Self {
//             container_name: netowrk_topology.container_name.clone(),
//             server_address: netowrk_topology.server_address.clone(),
//             client_names: netowrk_topology.client_connections.connection.clone().into_iter().map(|x| x.name).collect::<Vec<_>>(), // Does this work???
//             container_state,
//             container_state_notify,
//             console_rx,
//             log_file_name: format!("logs/{}_Log_{}", netowrk_topology.container_name.clone(), Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string())
//         }
//     }

//     pub async fn run_console(&mut self) -> Result<(), Box<dyn Error>>{
//         enable_raw_mode()?; // Enable raw mode for UI
//         let mut stdout = io::stdout();
//         execute!(stdout, EnterAlternateScreen)?; // Enter alternate screen for smooth UI
//         let backend = CrosstermBackend::new(stdout);
//         let mut terminal = Terminal::new(backend)?;

//         let mut logs: Vec<String> = vec!["Starting server...".to_string()];
//         log_to_file("Starting server...")?; // Log first message to file
//         let mut scroll_offset: usize = 0;

//         loop {
//             // Get terminal size and determine how many logs fit
//             let size = terminal.size()?;
//             let max_log_lines = size.height.saturating_sub(6) as usize; // Leave space for borders & status

//             // Render UI
//             terminal.draw(|f| {
//                 let chunks = Layout::default()
//                     .direction(Direction::Vertical)
//                     .constraints([Constraint::Length(5), Constraint::Min(5)].as_ref())
//                     .split(f.area());

//                 // Status Block
//                 let status_text = Paragraph::new("Server Status: RUNNING\nClient 1: CONNECTED\nClient 2: DISCONNECTED")
//                     .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
//                     .block(Block::default().borders(Borders::ALL).title(" System Status "));

//                 // Only show the logs that fit in the screen
//                 let visible_logs = if logs.len() > max_log_lines {
//                     &logs[scroll_offset..scroll_offset + max_log_lines.min(logs.len())]
//                 } else {
//                     &logs[..]
//                 };

//                 // Create List of Logs
//                 let log_items: Vec<ListItem> = visible_logs.iter().map(|log| {
//                     let color = if log.contains("ERROR") {
//                         Color::Red
//                     } else if log.contains("WARN") {
//                         Color::Yellow
//                     } else {
//                         Color::White
//                     };
//                     ListItem::new(Span::styled(log.clone(), Style::default().fg(color)))
//                 }).collect();

//                 // Scrollable Log List
//                 let log_list = List::new(log_items)
//                     .block(Block::default().borders(Borders::ALL).title(" Logs "))
//                     .highlight_symbol("> ");

//                 f.render_widget(status_text, chunks[0]);
//                 f.render_widget(log_list, chunks[1]);
//             })?;

//             // Simulate log updates every second
//             sleep(Duration::from_secs(1)).await;
//             let new_log = format!("INFO: New log entry at {:?}", chrono::Utc::now());
//             logs.push(new_log.clone());
//             log_to_file(&new_log)?; // Write new log to file

//             // Auto-scroll
//             scroll_offset = logs.len().saturating_sub(max_log_lines);

//             if ContainerState::from(self.container_state.load(Ordering::SeqCst)) == ContainerState::ShuttingDown {
//                 sleep(Duration::from_secs(7)).await;
//                 log::info!("{} Console shutting down...", self.container_name);
//                 sleep(Duration::from_secs(1)).await;
//                 break;
//             }
//         }

//         // Cleanup terminal
//         disable_raw_mode()?;
//         execute!(io::stdout(), LeaveAlternateScreen)?;
//         Ok(())
//     }
// }
