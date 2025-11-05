use std::{io::Write, path::PathBuf};

const DEFAULT_RELAYS_FILEPATH: &str = ".default_relays_path.txt";
const DEFAULT_KEY_PATH: &str = ".default_keypair_path.txt";

pub fn write_into_stdout<T: AsRef<str> + std::fmt::Debug>(text: T) -> std::io::Result<usize> {
    let mut output = text.as_ref().to_string();
    output.push('\n');
    std::io::stdout().write(output.as_bytes())
}

pub fn default_key_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_KEY_PATH)
}

pub fn default_relays_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_RELAYS_FILEPATH)
}
