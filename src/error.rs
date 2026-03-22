use anyhow::{bail, Result};

pub fn fail<T>(message: impl Into<String>) -> Result<T> {
    bail!(message.into())
}
