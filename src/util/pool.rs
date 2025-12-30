use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Pool<T: Clone + Send + Sync + 'static> {
    items: Arc<Mutex<VecDeque<T>>>,
}

impl<T: Clone + Send + Sync + 'static> FromIterator<T> for Pool<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl<T: Clone + Send + Sync + 'static> Pool<T> {
    pub fn new(items: VecDeque<T>) -> Self {
        Self {
            items: Arc::new(Mutex::new(items)),
        }
    }

    pub fn get_item(&mut self) -> impl Future<Output = Item<T>> {
        let pointer = self.items.clone();

        async move {
            loop {
                let mut items = pointer.lock().await;
                if let Some(item) = items.pop_front() {
                    return Item {
                        inner: item,
                        pointer: pointer.clone(),
                    };
                }
                drop(items);
                tokio::task::yield_now().await;
            }
        }
    }
}

#[derive(Debug)]
pub struct Item<T: Clone + Sync + Send + 'static> {
    inner: T,
    pointer: Arc<Mutex<VecDeque<T>>>,
}

impl<T: Clone + Send + Sync + 'static> Drop for Item<T> {
    fn drop(&mut self) {
        let p = self.pointer.clone();
        let item = self.inner.clone();

        tokio::spawn(async move {
            let mut items = p.lock().await;
            items.push_back(item);
        });
    }
}

impl<T: Clone + Send + Sync + 'static> Deref for Item<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: Clone + Send + Sync + 'static> DerefMut for Item<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: Clone + Send + Sync + 'static + fmt::Display> fmt::Display for Item<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool() {
        let items = vec![1, 2, 3];
        let mut pool = Pool::from_iter(items);
        let item1 = pool.get_item().await;
        let item2 = pool.get_item().await;
        let item3 = pool.get_item().await;
        assert_eq!(*item1, 1);
        assert_eq!(*item2, 2);
        assert_eq!(*item3, 3);
        drop(item1);
        let item4 = pool.get_item().await;
        assert_eq!(*item4, 1);
    }

    #[tokio::test]
    async fn test_pool_should_be_locked() {
        let items = vec![1];
        let mut pool = Pool::from_iter(items);
        let _item1 = pool.get_item().await;
        let get_item_future = pool.get_item();

        tokio::select! {
            _ = get_item_future => panic!("Should not get item"),
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => (),
        }
    }
}
