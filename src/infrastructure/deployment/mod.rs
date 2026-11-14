//! Run deployment transfers and credential storage for the interface and the command line.

pub(crate) mod transfer;
pub(crate) mod transport;

use crate::domain::deployment::Error;

/// Generate one random identifier for connections, keys, and profiles.
pub(crate) fn identifier() -> Result<String, Error> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Configuration)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
