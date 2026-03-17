// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! KovaStream — unified streaming adapter.
//! Adapts between broadcast (GUI/serve multi-subscriber), mpsc (agent loop), and stdout (CLI).

use std::sync::Arc;

/// Unified stream that can be consumed by any surface.
pub enum KovaStream {
    /// Broadcast channel — GUI and serve subscribe.
    Broadcast(tokio::sync::broadcast::Receiver<Arc<str>>),
    /// Mpsc channel — agent loop consumes sequentially.
    Mpsc(std::sync::mpsc::Receiver<Arc<str>>),
    /// Collected — all tokens in one string (blocking result).
    Collected(String),
}

impl KovaStream {
    /// Create from a broadcast receiver (pipeline pattern).
    pub fn from_broadcast(rx: tokio::sync::broadcast::Receiver<Arc<str>>) -> Self {
        KovaStream::Broadcast(rx)
    }

    /// Create from an mpsc receiver (inference pattern).
    pub fn from_mpsc(rx: std::sync::mpsc::Receiver<Arc<str>>) -> Self {
        KovaStream::Mpsc(rx)
    }

    /// Create from a completed string.
    pub fn from_string(s: String) -> Self {
        KovaStream::Collected(s)
    }

    /// Collect all tokens into a single string. Blocks until stream ends.
    pub fn collect_blocking(self) -> String {
        match self {
            KovaStream::Broadcast(mut rx) => {
                let mut out = String::new();
                while let Ok(token) = rx.blocking_recv() {
                    out.push_str(&token);
                }
                out
            }
            KovaStream::Mpsc(rx) => {
                let mut out = String::new();
                for token in rx {
                    out.push_str(&token);
                }
                out
            }
            KovaStream::Collected(s) => s,
        }
    }

    /// Print tokens to stdout as they arrive. Returns full text.
    pub fn to_stdout(self) -> String {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        match self {
            KovaStream::Broadcast(mut rx) => {
                let mut out = String::new();
                while let Ok(token) = rx.blocking_recv() {
                    print!("{}", &*token);
                    let _ = stdout.flush();
                    out.push_str(&token);
                }
                println!();
                out
            }
            KovaStream::Mpsc(rx) => {
                let mut out = String::new();
                for token in rx {
                    print!("{}", &*token);
                    let _ = stdout.flush();
                    out.push_str(&token);
                }
                println!();
                out
            }
            KovaStream::Collected(s) => {
                println!("{}", s);
                s
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collected_stream() {
        let stream = KovaStream::from_string("hello world".into());
        assert_eq!(stream.collect_blocking(), "hello world");
    }

    #[test]
    fn mpsc_stream() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Arc::from("hello ")).unwrap();
        tx.send(Arc::from("world")).unwrap();
        drop(tx);
        let stream = KovaStream::from_mpsc(rx);
        assert_eq!(stream.collect_blocking(), "hello world");
    }
}
