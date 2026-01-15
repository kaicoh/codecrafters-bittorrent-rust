use crate::util::{KeyHash, ThrottleQueue};

use super::{PeerMessage, PeerStream, Piece, PieceManager};

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::{
    Mutex,
    mpsc::{self, Receiver},
};
use tokio_stream::StreamExt;
use tracing::{debug, error};

const BLOCK_SIZE: usize = 16 * 1024;
const THROTTLE_CAPACITY: usize = 5;

type PeerMessageSender =
    Box<dyn Fn(PeerMessage) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

type Queue = Arc<Mutex<ThrottleQueue<PeerMessage, PeerMessageSender>>>;
type Pieces = Arc<Mutex<PieceManager>>;

pub struct Broker {
    queue: Queue,
    pieces: Pieces,
}

impl Broker {
    pub fn new(stream: PeerStream) -> (Self, Receiver<Piece>) {
        let PeerStream {
            mut reader, writer, ..
        } = stream;

        let writer = Arc::new(Mutex::new(writer));

        let queue = Arc::new(Mutex::new(ThrottleQueue::new(
            THROTTLE_CAPACITY,
            send_message(writer),
        )));

        let queue_pointer = Arc::clone(&queue);

        let (piece_tx, piece_rx) = mpsc::channel::<Piece>(100);
        let piece_manager = PieceManager::new(piece_tx);

        let pieces = Arc::new(Mutex::new(piece_manager));
        let pieces_pointer = Arc::clone(&pieces);

        tokio::spawn(async move {
            while let Some(msg) = reader.next().await {
                let msg = match msg {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Failed to read message: {e}");
                        break;
                    }
                };

                let mut queue = queue_pointer.lock().await;
                queue.done(msg.key_hash()).await;

                if let PeerMessage::Piece {
                    index,
                    begin,
                    block,
                } = msg
                {
                    debug!(
                        "Received piece message: index={}, begin={}, block_length={}",
                        index,
                        begin,
                        block.len()
                    );
                    let mut pieces = pieces_pointer.lock().await;

                    if let Err(err) = pieces
                        .insert_block(index as usize, begin as usize, block)
                        .await
                    {
                        error!("Failed to insert block: {err}");
                        break;
                    }
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
        self.queue.lock().await.queue(msg).await;
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

fn send_message(writer: Arc<Mutex<OwnedWriteHalf>>) -> PeerMessageSender {
    Box::new(move |msg: PeerMessage| {
        let writer = Arc::clone(&writer);

        Box::pin(async move {
            let bytes = msg.into_bytes();
            let mut writer = writer.lock().await;
            if let Err(err) = writer.write_all(&bytes).await {
                error!("Failed to send message: {err}");
            }
        })
    })
}
