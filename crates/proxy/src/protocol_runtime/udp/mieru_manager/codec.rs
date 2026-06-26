use zero_core::Address;
use zero_core::Error;
use zero_engine::EngineError;
use zero_traits::DatagramCodec;

pub(super) fn packet(
    codec: &dyn DatagramCodec<Address, Error = Error>,
    target: &Address,
    target_port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, EngineError> {
    codec
        .encode(target, target_port, payload)
        .map_err(EngineError::from)
}

pub(super) fn decode_packet(
    codec: &dyn DatagramCodec<Address, Error = Error>,
    payload: &[u8],
) -> Result<(Address, u16, Vec<u8>), EngineError> {
    codec.decode(payload).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "failed to decode Mieru UDP flow datagram",
        ))
    })
}
