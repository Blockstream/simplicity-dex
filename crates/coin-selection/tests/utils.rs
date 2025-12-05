use global_utils::logger::{LoggerGuard, init_logger};
use std::sync::LazyLock;

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(init_logger);
