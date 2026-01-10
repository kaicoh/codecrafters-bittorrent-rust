use crate::util::{KeyHash, ThrottleQueue};

use super::{PeerMessage, Piece, PieceManager};

use std::sync::Arc;
use tokio::sync::{
    Mutex,
    mpsc::{self, Receiver, Sender},
};

const BLOCK_SIZE: usize = 16 * 1024;
const THROTTLE_CAPACITY: usize = 5;

type Queue = Arc<Mutex<ThrottleQueue<PeerMessage, Box<dyn Fn(PeerMessage) + Send + Sync>>>>;
type Pieces = Arc<Mutex<PieceManager>>;

pub struct Broker {
    queue: Queue,
    pieces: Pieces,
}

impl Broker {
    pub fn new(
        sender: Sender<PeerMessage>,
        mut receiver: Receiver<PeerMessage>,
    ) -> (Self, Receiver<Piece>) {
        let send_queue = send_message(sender);

        let mut queue = ThrottleQueue::new(THROTTLE_CAPACITY, send_queue);
        queue.add_skip(PeerMessage::Choke.key_hash());

        let queue = Arc::new(Mutex::new(queue));
        let queue_pointer = Arc::clone(&queue);

        let (piece_tx, piece_rx) = mpsc::channel::<Piece>(100);
        let send_piece = send_piece(piece_tx);

        let piece_manager = PieceManager::new(send_piece);

        let pieces = Arc::new(Mutex::new(piece_manager));
        let pieces_pointer = Arc::clone(&pieces);

        tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                let mut queue = queue_pointer.lock().await;
                queue.done(msg.key_hash());

                if let PeerMessage::Piece {
                    index,
                    begin,
                    block,
                } = msg
                {
                    let mut pieces = pieces_pointer.lock().await;
                    pieces.insert_block(index as usize, begin as usize, block);
                }
            }
        });

        (Self { queue, pieces }, piece_rx)
    }

    pub async fn request_piece(&mut self, index: usize, piece_length: usize) {
        self.new_piece(index, piece_length).await;
        self.send_piece_request(index, piece_length).await;
    }

    async fn queue(&mut self, msg: PeerMessage) {
        self.queue.lock().await.queue(msg);
    }

    async fn new_piece(&mut self, index: usize, length: usize) {
        self.pieces.lock().await.new_block(index, length);
    }

    async fn send_piece_request(&mut self, index: usize, piece_length: usize) {
        let mut offset = 0;

        while offset < piece_length {
            let block_size = std::cmp::min(BLOCK_SIZE, piece_length - offset);

            let request_msg = PeerMessage::Request {
                index: index as u32,
                begin: offset as u32,
                length: block_size as u32,
            };

            self.queue(request_msg).await;

            offset += block_size;
        }
    }
}

fn send_message(sender: Sender<PeerMessage>) -> Box<dyn Fn(PeerMessage) + Send + Sync> {
    Box::new(move |msg: PeerMessage| {
        let sender = sender.clone();

        tokio::spawn(async move {
            if let Err(e) = sender.send(msg).await {
                eprintln!("Failed to send message: {e}");
            }
        });
    })
}

fn send_piece(sender: Sender<Piece>) -> Box<dyn Fn(Piece) + Send + Sync> {
    Box::new(move |piece: Piece| {
        let sender = sender.clone();

        tokio::spawn(async move {
            if let Err(e) = sender.send(piece).await {
                eprintln!("Failed to send piece: {e}");
            }
        });
    })
}
