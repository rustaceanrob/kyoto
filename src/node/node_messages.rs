use bitcoin::Block;

use crate::tx::types::IndexedTransaction;

#[derive(Debug)]
pub enum NodeMessage {
    Dialog(String),
    Warning(String),
    Transaction(IndexedTransaction),
    Block(Block),
    Synced,
}

#[derive(Debug)]
pub enum ClientMessage {
    Shutdown,
}
