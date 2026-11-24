use core::fmt;

use embassy_stm32::{
    peripherals::{DMA2_CH3, SDMMC1},
    sdmmc::{self, DataBlock, Sdmmc},
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx};

/// Error returned by the SD card block device.
#[derive(Debug)]
pub struct HypedSdmmcError(pub sdmmc::Error);

impl fmt::Display for HypedSdmmcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SDMMC error: {:?}", self.0)
    }
}

impl core::error::Error for HypedSdmmcError {}

impl From<sdmmc::Error> for HypedSdmmcError {
    fn from(err: sdmmc::Error) -> Self {
        Self(err)
    }
}

/// SD card block device for the STM32F767ZI, used by embedded-sdmmc.
pub struct HypedSdmmc {
    sdmmc: Mutex<NoopRawMutex, Sdmmc<'static, SDMMC1, DMA2_CH3>>,
}

impl HypedSdmmc {
    /// Wraps an Sdmmc driver whose card has already been set up with `init_card`.
    pub fn new(sdmmc: Sdmmc<'static, SDMMC1, DMA2_CH3>) -> Self {
        Self {
            sdmmc: Mutex::new(sdmmc),
        }
    }
}

impl BlockDevice for HypedSdmmc {
    type Error = HypedSdmmcError;

    async fn read(
        &self,
        blocks: &mut [Block],
        start_block_idx: BlockIdx,
    ) -> Result<(), Self::Error> {
        let mut sdmmc = self.sdmmc.lock().await;
        let mut buffer = DataBlock([0u8; Block::LEN]);
        for (i, block) in blocks.iter_mut().enumerate() {
            sdmmc
                .read_block(start_block_idx.0 + i as u32, &mut buffer)
                .await?;
            block.contents.copy_from_slice(&buffer.0);
        }
        Ok(())
    }

    async fn write(&self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let mut sdmmc = self.sdmmc.lock().await;
        let mut buffer = DataBlock([0u8; Block::LEN]);
        for (i, block) in blocks.iter().enumerate() {
            buffer.0.copy_from_slice(&block.contents);
            sdmmc
                .write_block(start_block_idx.0 + i as u32, &buffer)
                .await?;
        }
        Ok(())
    }

    async fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        let sdmmc = self.sdmmc.lock().await;
        let size_bytes = sdmmc.card()?.size();
        Ok(BlockCount((size_bytes / Block::LEN as u64) as u32))
    }
}
