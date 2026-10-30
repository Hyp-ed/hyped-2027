#![no_std]

pub mod boards;
pub mod bus;
pub mod can_id;
pub mod data;
pub mod emergency;
pub mod events;
pub mod heartbeat;
pub mod measurements;
pub mod message_identifier;
pub mod messages;

pub fn test_minicbor() -> Result<(), minicbor::encode::Error<minicbor::encode::write::EndOfSlice>> {
    let input = ["hello", "world"];
    let mut buffer = [0u8; 128];

    minicbor::encode(input, buffer.as_mut())?;
    let output: [&str; 2] = minicbor::decode(buffer.as_ref()).expect("a");
    assert_eq!(input, output);

    Ok(())
}
