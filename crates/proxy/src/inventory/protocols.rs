use super::ProtocolInventory;
use crate::transport::DirectConnector;

impl ProtocolInventory {
    pub(crate) fn direct_connector(&self) -> DirectConnector {
        DirectConnector
    }
}
