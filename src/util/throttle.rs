use crate::util::Bytes20;
use std::collections::{HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;

pub struct ThrottleQueue<T, F>
where
    T: KeyHash,
    F: Fn(T) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync,
{
    waitings: VecDeque<T>,
    processings: HashSet<Bytes20>,
    capacity: usize,
    cb: Box<F>,
}

impl<T, F> ThrottleQueue<T, F>
where
    T: KeyHash,
    F: Fn(T) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync,
{
    pub fn new(capacity: usize, cb: F) -> Self {
        Self {
            waitings: VecDeque::new(),
            processings: HashSet::new(),
            capacity,
            cb: Box::new(cb),
        }
    }

    pub async fn queue(&mut self, item: T) -> Bytes20 {
        let hash = item.key_hash();

        if self.is_full() {
            self.push_waiting(item);
        } else {
            self.push(item).await;
        }

        hash
    }

    pub async fn done(&mut self, hash: Bytes20) {
        if self.processings.contains(&hash) {
            self.processings.remove(&hash);

            if let Some(item) = self.waitings.pop_front() {
                self.push(item).await;
            }
        } else {
            self.waitings.retain(|item| item.key_hash() == hash);
        }
    }

    fn is_full(&self) -> bool {
        self.processings.len() >= self.capacity
    }

    fn push_waiting(&mut self, item: T) {
        self.waitings.push_back(item);
    }

    async fn push(&mut self, item: T) {
        let hash = item.key_hash();
        self.processings.insert(hash);
        (self.cb)(item).await;
    }
}

pub trait KeyHash {
    fn key_hash(&self) -> Bytes20;
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha1::{Digest, Sha1};
    use std::sync::{Arc, Mutex};

    struct TestItem {
        data: Vec<u8>,
    }

    impl KeyHash for TestItem {
        fn key_hash(&self) -> Bytes20 {
            let digest = Sha1::digest(&self.data);
            Bytes20::from(digest.as_ref())
        }
    }

    #[tokio::test]
    async fn test_throttle_queue() {
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(vec![]));
        let p = Arc::clone(&buf);

        let mut queue = ThrottleQueue::new(2, move |item: TestItem| {
            let p = Arc::clone(&p);
            Box::pin(async move {
                let mut guard = p.lock().unwrap();
                guard.extend_from_slice(&item.data);
            })
        });

        assert!(buf.lock().unwrap().is_empty());

        let item1 = TestItem {
            data: vec![1, 2, 3],
        };
        let item2 = TestItem {
            data: vec![4, 5, 6],
        };
        let item3 = TestItem {
            data: vec![7, 8, 9],
        };

        let hash1 = queue.queue(item1).await;
        let hash2 = queue.queue(item2).await;
        let hash3 = queue.queue(item3).await;

        assert!(queue.processings.contains(&hash1));
        assert!(queue.processings.contains(&hash2));
        assert!(queue.waitings.len() == 1);
        assert_eq!(*buf.lock().unwrap(), vec![1, 2, 3, 4, 5, 6]);

        queue.done(hash1).await;

        assert!(queue.processings.contains(&hash2));
        assert!(queue.processings.contains(&hash3));
        assert!(queue.waitings.is_empty());
        assert_eq!(*buf.lock().unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}
