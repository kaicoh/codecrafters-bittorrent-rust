use crate::{BitTorrentError, Result};

use std::collections::HashMap;
use tokio::sync::mpsc::Sender;
use tracing::debug;

type Index = usize;
type Offset = usize;

#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub index: Index,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Blocks {
    index: Index,
    length: usize,
    blocks: HashMap<Offset, Vec<u8>>,
}

impl Blocks {
    pub fn new(index: Index, length: usize) -> Self {
        Self {
            index,
            length,
            blocks: HashMap::new(),
        }
    }

    pub fn insert_block(&mut self, begin: Offset, data: Vec<u8>) {
        self.blocks.insert(begin, data);
    }

    pub fn is_complete(&self) -> bool {
        self.len() == self.length
    }

    fn len(&self) -> usize {
        self.blocks.values().map(|b| b.len()).sum()
    }

    fn into_piece(self) -> Option<Piece> {
        if !self.is_complete() {
            return None;
        }

        let mut data = vec![0u8; self.length];
        for (begin, block) in self.blocks {
            data[begin..begin + block.len()].copy_from_slice(&block);
        }

        Some(Piece {
            index: self.index,
            data,
        })
    }
}

pub struct PieceManager {
    blocks: HashMap<Index, Blocks>,
    sender: Sender<Piece>,
}

impl PieceManager {
    pub fn new(sender: Sender<Piece>) -> Self {
        Self {
            blocks: HashMap::new(),
            sender,
        }
    }

    pub fn new_block(&mut self, index: Index, piece_length: usize) {
        self.blocks.insert(index, Blocks::new(index, piece_length));
    }

    pub async fn insert_block(&mut self, index: Index, begin: Offset, data: Vec<u8>) -> Result<()> {
        if let Some(blocks) = self.blocks.get_mut(&index) {
            blocks.insert_block(begin, data);
        }

        if self.blocks.get(&index).is_some_and(|b| b.is_complete())
            && let Some(piece) = self.blocks.remove(&index).and_then(|b| b.into_piece())
        {
            debug!("Piece {index} completed.");

            self.sender
                .send(piece)
                .await
                .map_err(|_| BitTorrentError::ChannelClosed)?;
        }

        Ok(())
    }
}
