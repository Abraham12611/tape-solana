use steel::*;
use crate::types::*;
use crate::state;
use super::AccountType;

#[repr(C)] 
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Writer {
    pub spool: Pubkey,
    pub state: SegmentTree, 
}

state!(AccountType, Writer);
