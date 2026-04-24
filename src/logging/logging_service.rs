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

// Package
use chrono::Utc;
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
                Level::Error => format!("{:<5}", "ERROR").red(),
                Level::Warn => format!("{:<5}", "WARN").yellow(),
                Level::Info => format!("{:<5}", "INFO").green(),
                Level::Debug => format!("{:<5}", "DEBUG").bright_cyan(),
                Level::Trace => format!("{:<5}", "TRACE").blue(),
            };

            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S");

            let module = record
                .module_path()
                .and_then(|m| m.split("::").last())
                .unwrap_or("unknown");

            println!("[{timestamp}] {level} {module:<20} - {}", record.args());
        }
    }

    fn flush(&self) {}
}

// Function to initialise the logger - not part of the struct
pub fn initialise_logger(log_level: LevelFilter) -> Result<(), SetLoggerError> {
    let logger = LoggingService {
        min_log_level: log_level,
    };

    // Set the logger as the global logger
    log::set_max_level(logger.min_log_level);
    log::set_boxed_logger(Box::new(logger))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::{Level, Log};

    fn make_service(level: LevelFilter) -> LoggingService {
        LoggingService { min_log_level: level }
    }

    #[test]
    fn enabled_passes_at_or_above_min_level() {
        let svc = make_service(LevelFilter::Info);
        assert!(svc.enabled(&log::Metadata::builder().level(Level::Error).target("t").build()));
        assert!(svc.enabled(&log::Metadata::builder().level(Level::Warn).target("t").build()));
        assert!(svc.enabled(&log::Metadata::builder().level(Level::Info).target("t").build()));
    }

    #[test]
    fn enabled_filters_below_min_level() {
        let svc = make_service(LevelFilter::Info);
        assert!(!svc.enabled(&log::Metadata::builder().level(Level::Debug).target("t").build()));
        assert!(!svc.enabled(&log::Metadata::builder().level(Level::Trace).target("t").build()));
    }

    #[test]
    fn log_does_not_panic_for_all_levels() {
        let svc = make_service(LevelFilter::Trace);
        // format_args! temporaries must be inlined — they cannot be stored in a let binding.
        svc.log(&log::Record::builder().level(Level::Error).target("t").module_path(Some("test::module")).args(format_args!("error message")).build());
        svc.log(&log::Record::builder().level(Level::Warn).target("t").module_path(Some("test::module")).args(format_args!("warn message")).build());
        svc.log(&log::Record::builder().level(Level::Info).target("t").module_path(Some("test::module")).args(format_args!("info message")).build());
        svc.log(&log::Record::builder().level(Level::Debug).target("t").module_path(Some("test::module")).args(format_args!("debug message")).build());
        svc.log(&log::Record::builder().level(Level::Trace).target("t").module_path(Some("test::module")).args(format_args!("trace message")).build());
    }

    #[test]
    fn log_silently_skips_filtered_levels() {
        let svc = make_service(LevelFilter::Error);
        svc.log(&log::Record::builder().level(Level::Trace).target("t").args(format_args!("should be skipped")).build()); // Must not panic
    }

    #[test]
    fn log_handles_missing_module_path() {
        let svc = make_service(LevelFilter::Trace);
        svc.log(&log::Record::builder().level(Level::Info).target("t").module_path(None).args(format_args!("no module path")).build()); // Should fall back to "unknown"
    }
}
