use core::fmt::Display;

#[derive(Debug)]
pub enum LogLevel {
    DEBUG,
    VERBOSE,
    INFO,
    WARNING,
    ERROR,
    FATAL,
}
pub(crate) fn _print(log_level: LogLevel, args: core::fmt::Arguments) {
    let cpu = super::arch::get_current_cpu();
    crate::println!("[C:{:03}][{}]: {}", cpu, log_level, args);
    crate::console_println!("[C:{:03}][{}]: {}", cpu, log_level, args);
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LogLevel::DEBUG => write!(f, "DEBUG  "),
            LogLevel::VERBOSE => write!(f, "VERBOSE"),
            LogLevel::INFO => write!(f, "INFO   "),
            LogLevel::WARNING => write!(f, "WARNING"),
            LogLevel::ERROR => write!(f, "ERROR  "),
            LogLevel::FATAL => write!(f, "FATAL  "),
        }
    }
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::logging::_print($crate::logging::LogLevel::DEBUG, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! verbose {
    ($($arg:tt)*) => {
        $crate::logging::_print($crate::logging::LogLevel::VERBOSE, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::logging::_print($crate::logging::LogLevel::INFO, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::logging::_print($crate::logging::LogLevel::WARNING, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::logging::_print($crate::logging::LogLevel::ERROR, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! fatal {
    ($($arg:tt)*) => {
        $crate::logging::_print($crate::logging::LogLevel::FATAL, format_args!($($arg)*));
    };
}
