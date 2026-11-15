//! Check an admitted retry before resolving an expired device-local picker choice again.

use std::time::Instant;

use carlos_proto::commands::RpcError;
use carlos_proto::ids::ClientId;
use carlos_proto::submissions::{ClientOperation, OperationResult};

use super::{error, expired, RetryStore};

impl RetryStore {
    /// Return a retained exact request, or authorize read-only preparation before its first admission.
    pub fn existing(
        &mut self,
        client: &ClientId,
        operation: &ClientOperation,
        canonical: &[u8],
        now: Instant,
    ) -> Result<Option<OperationResult>, RpcError> {
        self.authorize(client, operation, now)?;
        if let Some(record) = self.records.get(operation) {
            if record.canonical != canonical {
                return Err(error(
                    -32005,
                    "operation counter already names different work",
                ));
            }
            return Ok(Some(record.result.clone()));
        }
        if operation.id.get() <= self.clients.records[client].high_water {
            return Err(expired());
        }
        Ok(None)
    }
}
