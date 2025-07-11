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
// Package
use colored::*;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};

// Local
// use crate::utilities::tx_sender::add_to_tx_with_retry;

// Define a simple logger that will handle logging messages with colours
pub struct LoggingService {
    pub min_log_level: LevelFilter,
    // pub console_tx: mpsc::Sender<String>
}

impl log::Log for LoggingService {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level().to_level_filter() <= self.min_log_level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = match record.level() {
                Level::Error => "ERROR".red(),
                Level::Warn => "WARN".yellow(),
                Level::Info => "INFO".green(),
                Level::Debug => "DEBUG".bright_cyan(),
                Level::Trace => "TRACE".blue(),
            };

            // Create the log message
            let log_message = format!("{} - {}", level, record.args());
            println!("{log_message}");

            // Send the log message to the UI (via the channel) - no wait lol
            // add_to_tx_with_retry(&self.console_tx, &log_message, "LoggingService", "Console");
        }
    }

    fn flush(&self) {}
}

// impl Logger {
//     // Method to send status updates
//     pub fn status_update(&self, entity_type: Type, id: &str, status: Status) {
//         let status_update = StatusUpdate {
//             entity_type,
//             id: id.to_string(),
//             status,
//         };

//         // Send status update to the UI
//         if let Err(e) = self.tx.send(status_update.to_string()) {
//             eprintln!("Error sending status update to UI: {e:?}");
//         }
//     }
// }

// Function to initialise the logger - not part of the struct
// Use optioin so that the log level can be set or left as default
pub fn initialise_logger(log_level: Option<LevelFilter>) -> Result<(), SetLoggerError> {
    let logger = LoggingService {
        min_log_level: log_level.unwrap_or(LevelFilter::Debug),
    };

    // Set the logger as the global logger
    log::set_max_level(logger.min_log_level); // No need to clone, `LevelFilter` is `Copy`
    log::set_boxed_logger(Box::new(logger))?;
    Ok(())
}
