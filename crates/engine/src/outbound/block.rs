use zero_core::Error;

#[derive(Debug, Default, Clone, Copy)]
pub struct BlockOutbound;

impl BlockOutbound {
    pub fn reject(&self) -> Result<(), Error> {
        Err(Error::Route("connection blocked by outbound"))
    }
}
