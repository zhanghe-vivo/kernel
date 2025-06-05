use crate::{clock::tick_get_millisecond, cpu::Cpu, println};
use log::{Level, LevelFilter, Metadata, Record};

struct Logger;

pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

///set max log level
pub fn set_max_level(level: LogLevel) {
    match level {
        LogLevel::Trace => log::set_max_level(LevelFilter::Trace),
        LogLevel::Debug => log::set_max_level(LevelFilter::Debug),
        LogLevel::Info => log::set_max_level(LevelFilter::Info),
        LogLevel::Warn => log::set_max_level(LevelFilter::Warn),
        LogLevel::Error => log::set_max_level(LevelFilter::Error),
    }
}

/// log init
pub fn logger_init() {
    static LOGGER: Logger = Logger {};
    #[cfg(debug)]
    log::set_max_level(LevelFilter::Trace);
    #[cfg(release)]
    log::set_max_level(LevelFilter::Warn);
    log::set_logger(&LOGGER).unwrap();
}

///impl log for Logger
impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let timestamp = tick_get_millisecond();
        let tid = unsafe { Cpu::get_current_thread().unwrap().as_mut().tid() };
        println!(
            "[{} ms] [TID:{}] {} : {}",
            timestamp,
            tid,
            record.level(),
            record.args()
        );
    }

    fn flush(&self) {}
}
