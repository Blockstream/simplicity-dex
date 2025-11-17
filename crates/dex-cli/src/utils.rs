use nostr::{Keys, RelayUrl};
use std::collections::HashSet;
use std::io::BufRead;
use std::str::FromStr;
use std::{io::Write, path::PathBuf};

const DEFAULT_RELAYS_FILEPATH: &str = ".simplicity-dex/relays.txt";
const DEFAULT_KEY_PATH: &str = ".simplicity-dex/keypair.txt";
pub const DEFAULT_CLIENT_TIMEOUT_SECS: u64 = 10;

pub fn write_into_stdout<T: AsRef<str> + std::fmt::Debug>(text: T) -> std::io::Result<usize> {
    let mut output = text.as_ref().to_string();
    output.push('\n');
    std::io::stdout().write(output.as_bytes())
}

pub fn default_key_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("../../.."))
        .join(DEFAULT_KEY_PATH)
}

pub fn default_relays_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("../../.."))
        .join(DEFAULT_RELAYS_FILEPATH)
}
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("Unable to parse url: {1}, error: {0}")]
    UrlParseError(nostr::types::url::Error, String),
    #[error("Got error on reading/writing to file: {1}, error: {0}")]
    ProblemWithFile(std::io::Error, PathBuf),
    #[error("Incorrect path to the file, please check validity of the path (err: path is not a file), got path: {0}")]
    IncorrectPathToFile(PathBuf),
    #[error("File is empty, got path: {0}")]
    EmptyFile(PathBuf),
    #[error("File is empty, got path: {0}")]
    KeyParseError(nostr::key::Error, String),
}

pub fn check_file_existence(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);

    if path.is_file() {
        Ok(path)
    } else {
        Err(FileError::IncorrectPathToFile(path.clone()).to_string())
    }
}

pub fn get_valid_urls_from_file(filepath: &PathBuf) -> Result<Vec<RelayUrl>, FileError> {
    let file = std::fs::File::open(filepath).map_err(|x| FileError::ProblemWithFile(x, filepath.clone()))?;
    let reader = std::io::BufReader::new(file);
    let mut set = HashSet::new();
    for x in reader.lines() {
        let line = x.map_err(|x| FileError::ProblemWithFile(x, filepath.clone()))?;
        match RelayUrl::parse(&line) {
            Ok(url) => {
                set.insert(url);
            }
            Err(e) => {
                return Err(FileError::UrlParseError(e, line));
            }
        }
    }
    Ok(set.into_iter().collect::<Vec<RelayUrl>>())
}

pub fn get_valid_key_from_file(filepath: &PathBuf) -> Result<Keys, FileError> {
    let file = std::fs::File::open(filepath).map_err(|x| FileError::ProblemWithFile(x, filepath.clone()))?;
    let reader = std::io::BufReader::new(file);
    let key = reader
        .lines()
        .next()
        .ok_or_else(|| FileError::EmptyFile(filepath.clone()))?
        .map_err(|x| FileError::ProblemWithFile(x, filepath.clone()))?;
    let key = Keys::from_str(&key).map_err(|e| FileError::KeyParseError(e, key))?;
    Ok(key)
}
