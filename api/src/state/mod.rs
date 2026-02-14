mod archive;
mod epoch;
mod block;
mod spool;
mod treasury;
mod writer;
mod miner;
mod reel;

pub use archive::*;
pub use epoch::*;
pub use block::*;
pub use spool::*;
pub use treasury::*;
pub use writer::*;
pub use miner::*;
pub use reel::*;

use steel::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum AccountType {
    Unknown = 0,
    Archive,
    Reel,
    Writer,
    Spool,
    Miner,
    Epoch,
    Block,
    Treasury,
}
