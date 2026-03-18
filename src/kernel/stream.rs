//! T206 — unified streaming adapter.
//! Adapts between broadcast (GUI/serve multi-subscriber), mpsc (agent loop), and stdout (CLI).

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::sync::Arc;

/// Unified stream that can be consumed by any surface.
pub enum T206 {
    /// Broadcast channel — GUI and serve subscribe.
    Broadcast(tokio::sync::broadcast::Receiver<Arc<str>>),
    /// Mpsc channel — agent loop consumes sequentially.
    Mpsc(std::sync::mpsc::Receiver<Arc<str>>),
    /// Collected — all tokens in one string (blocking result).
    Collected(String),
}

impl T206 {
    /// Create from a broadcast receiver (pipeline pattern).
    pub fn from_broadcast(rx: tokio::sync::broadcast::Receiver<Arc<str>>) -> Self {
        T206::Broadcast(rx)
    }

    /// Create from an mpsc receiver (inference pattern).
    pub fn from_mpsc(rx: std::sync::mpsc::Receiver<Arc<str>>) -> Self {
        T206::Mpsc(rx)
    }

    /// Create from a completed string.
    pub fn from_string(s: String) -> Self {
        T206::Collected(s)
    }

    /// Collect all tokens into a single string. Blocks until stream ends.
    pub fn collect_blocking(self) -> String {
        match self {
            T206::Broadcast(mut rx) => {
                let mut out = String::new();
                while let Ok(token) = rx.blocking_recv() {
                    out.push_str(&token);
                }
                out
            }
            T206::Mpsc(rx) => {
                let mut out = String::new();
                for token in rx {
                    out.push_str(&token);
                }
                out
            }
            T206::Collected(s) => s,
        }
    }

    /// Print tokens to stdout as they arrive. Returns full text.
    pub fn to_stdout(self) -> String {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        match self {
            T206::Broadcast(mut rx) => {
                let mut out = String::new();
                while let Ok(token) = rx.blocking_recv() {
                    print!("{}", &*token);
                    let _ = stdout.flush();
                    out.push_str(&token);
                }
                println!();
                out
            }
            T206::Mpsc(rx) => {
                let mut out = String::new();
                for token in rx {
                    print!("{}", &*token);
                    let _ = stdout.flush();
                    out.push_str(&token);
                }
                println!();
                out
            }
            T206::Collected(s) => {
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
        let stream = T206::from_string("hello world".into());
        assert_eq!(stream.collect_blocking(), "hello world");
    }

    #[test]
    fn mpsc_stream() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Arc::from("hello ")).unwrap();
        tx.send(Arc::from("world")).unwrap();
        drop(tx);
        let stream = T206::from_mpsc(rx);
        assert_eq!(stream.collect_blocking(), "hello world");
    }
}