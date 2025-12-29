use std::collections::VecDeque;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

#[derive(Debug, Clone)]
pub struct Pool<T: Clone> {
    items: VecDeque<T>,
}

impl<T: Clone> FromIterator<T> for Pool<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            items: iter.into_iter().collect(),
        }
    }
}

impl<T: Clone> Pool<T> {
    pub fn new(items: VecDeque<T>) -> Self {
        Self { items }
    }

    pub fn get_item(&mut self) -> ItemFuture<T> {
        ItemFuture {
            pointer: Arc::new(Mutex::new(self.clone())),
        }
    }

    fn add_item(&mut self, item: T) {
        self.items.push_back(item);
    }
}

#[derive(Debug)]
pub struct ItemFuture<T: Clone> {
    pointer: Arc<Mutex<Pool<T>>>,
}

impl<T: Clone> Future for ItemFuture<T> {
    type Output = Item<T>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut pool = self.pointer.lock().expect("Failed to lock the pool");
        if let Some(item) = pool.items.pop_front() {
            Poll::Ready(Item {
                inner: item,
                pointer: self.pointer.clone(),
            })
        } else {
            Poll::Pending
        }
    }
}

#[derive(Debug)]
pub struct Item<T: Clone> {
    inner: T,
    pointer: Arc<Mutex<Pool<T>>>,
}

impl<T: Clone> Drop for Item<T> {
    fn drop(&mut self) {
        let mut pool = self.pointer.lock().expect("Failed to lock the pool");
        pool.add_item(self.inner.clone());
    }
}

impl<T: Clone> Deref for Item<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: Clone> DerefMut for Item<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
