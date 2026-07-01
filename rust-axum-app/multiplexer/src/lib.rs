use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::broadcast::{self, error::SendError};

#[derive(Debug, Clone)]
pub struct Message {
    pub bytes: Bytes,
    pub offset: usize,
}

impl Message {
    pub fn random() -> Self {
        let bytes = Bytes::from_static(b"Hello");
        let offset = 0;

        Self { bytes, offset }
    }
    pub fn random2() -> Self {
        let bytes = Bytes::from_static(b"Hello2");
        let offset = 0;

        Self { bytes, offset }
    }
}

pub struct Multiplexer {
    channels: DashMap<String, broadcast::Sender<Message>>,
    capacity: usize,
}
impl Multiplexer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            channels: DashMap::new(),
        }
    }

    pub fn subscribe(&self, url: String) -> broadcast::Receiver<Message> {
        let sender = self.channels.entry(url).or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(self.capacity);
            tx
        });

        sender.subscribe()
    }

    pub fn publish(&self, url: String, msg: Message) -> Result<usize, SendError<Message>> {
        if let Some(sender) = self.channels.get(&url) {
            sender.send(msg)
        } else {
            Err(SendError(msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};
    use tokio::time::sleep;

    use super::*;

    #[tokio::test]
    async fn test_multiplexer() {
        let mux = Arc::new(Multiplexer::new(100));

        let mux_clone_1 = mux.clone();

        tokio::spawn(async move {
            let mut rx = mux_clone_1.subscribe("system.events".to_string());

            while let Ok(msg) = rx.recv().await {
                println!(
                    "Subscriber 1 received message on 'system.events': {:?}",
                    msg
                )
            }
        });

        let mux_clone_2 = mux.clone();
        tokio::spawn(async move {
            let mut rx = mux_clone_2.subscribe("user.events".to_string());

            while let Ok(msg) = rx.recv().await {
                println!(
                    "Subscriber 2 received a message on 'user.events': {:?}",
                    msg
                )
            }
        });
        let mux_clone_3 = mux.clone();
        tokio::spawn(async move {
            let mut rx = mux_clone_3.subscribe("system.events".to_string());
            sleep(Duration::from_millis(80)).await;

            while let Ok(msg) = rx.recv().await {
                println!(
                    "Subscriber 3 received a message on 'system.events': {:?}",
                    msg
                )
            }
        });

        sleep(Duration::from_millis(50)).await;

        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("system.events".to_string(), Message::random());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());
        let _ = mux.publish("user.events".to_string(), Message::random2());

        sleep(Duration::from_millis(100)).await;
    }
}
