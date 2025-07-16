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

// Define a simple logger that will handle logging messages with colours
pub struct LoggingService {
    pub min_log_level: LevelFilter,
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
        }
    }

    fn flush(&self) {}
}

// Function to initialise the logger - not part of the struct
// Use option so that the log level can be set or left as default
pub fn initialise_logger(log_level: Option<LevelFilter>) -> Result<(), SetLoggerError> {
    let logger = LoggingService {
        min_log_level: log_level.unwrap_or(LevelFilter::Debug),
    };

    // Set the logger as the global logger
    log::set_max_level(logger.min_log_level);
    log::set_boxed_logger(Box::new(logger))?;
    Ok(())
}
