//! The guest's own type of `flm:host`: [`HostHandle`], a host service by the
//! kind of its handle.

use serde_json::Value;

use crate::adapter::handles::HandleKind;
use crate::adapter::values::{handle_from_value, JsonForm};
use crate::guest::host::{Files, HostService};
use crate::runtime::ExtError;

/// Mirrors [`HostHandle`](crate::api::host::HostHandle): a service of the
/// host, the reader of files or a named service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostHandle {
    /// The reader of files.
    Files(Files),
    /// A named service.
    Service(HostService),
}

/// The handle itself, of the kind `files` or `host_service`.
impl JsonForm for HostHandle {
    fn to_value(&self) -> Value {
        match self {
            HostHandle::Files(files) => files.to_value(),
            HostHandle::Service(service) => service.to_value(),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let handle = handle_from_value(value)?;
        match handle.kind {
            HandleKind::Files => Ok(HostHandle::Files(Files { handle })),
            HandleKind::HostService => Ok(HostHandle::Service(HostService { handle })),
            other => Err(ExtError::Argument {
                method: String::new(),
                detail: format!("an object of the kind `files` or `host_service` is expected here, not one of the kind `{}`", other.name()),
            }),
        }
    }
}
