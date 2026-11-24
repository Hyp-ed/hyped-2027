use heapless::Vec;

pub struct Log {
  pub board: u8,
  pub data: Vec<u8, 16>,
}

pub struct Warning {
  pub board: u8,
  pub data: Vec<u8, 16>,
}