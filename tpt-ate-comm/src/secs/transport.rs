//! In-memory full-duplex byte pipe with the same blocking `Read`/`Write`
//! semantics as a TCP stream, used to connect the equipment simulator to a
//! controller without real network I/O.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct PipeInner {
    buffer: VecDeque<u8>,
    /// Writer has called close (or was dropped); reads drain then report EOF.
    closed: bool,
}

struct Endpoint {
    inner: Mutex<PipeInner>,
    readable: Condvar,
}

impl Endpoint {
    fn write_bytes(&self, data: &[u8]) -> std::io::Result<usize> {
        let mut inner = self.inner.lock().expect("pipe mutex");
        if inner.closed {
            return Err(std::io::Error::new(ErrorKind::BrokenPipe, "pipe closed"));
        }
        inner.buffer.extend(data);
        self.readable.notify_all();
        Ok(data.len())
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn read_bytes(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut inner = self.inner.lock().expect("pipe mutex");
        loop {
            if !inner.buffer.is_empty() {
                let n = buf.len().min(inner.buffer.len());
                for slot in buf.iter_mut().take(n) {
                    *slot = inner.buffer.pop_front().expect("length checked");
                }
                return Ok(n);
            }
            if inner.closed {
                return Ok(0);
            }
            inner = self.readable.wait(inner).expect("condvar wait on pipe");
        }
    }

    fn close(&self) {
        let mut inner = self.inner.lock().expect("pipe mutex");
        inner.closed = true;
        self.readable.notify_all();
    }
}

/// One direction of a [`duplex`] pair. Writes go to the peer; reads consume
/// what the peer wrote.
pub struct DuplexHalf {
    local: Arc<Endpoint>,
    peer: Arc<Endpoint>,
}

impl Write for DuplexHalf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.peer.write_bytes(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.peer.flush()
    }
}

impl Read for DuplexHalf {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.local.read_bytes(buf)
    }
}

impl Drop for DuplexHalf {
    fn drop(&mut self) {
        self.peer.close();
    }
}

/// Create a connected pair of in-memory byte pipes. Writes on one half are
/// readable from the other; blocking behaves like a connected TCP socket.
pub fn duplex() -> (DuplexHalf, DuplexHalf) {
    let a =
        Arc::new(Endpoint { inner: Mutex::new(PipeInner::default()), readable: Condvar::new() });
    let b =
        Arc::new(Endpoint { inner: Mutex::new(PipeInner::default()), readable: Condvar::new() });
    (
        DuplexHalf { local: Arc::clone(&a), peer: Arc::clone(&b) },
        DuplexHalf { local: Arc::clone(&b), peer: Arc::clone(&a) },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn bytes_flow_both_ways() {
        let (mut a, mut b) = duplex();
        std::io::Write::write_all(&mut a, b"hello").unwrap();
        let mut buf = [0u8; 5];
        b.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hello");
        std::io::Write::write_all(&mut b, b"qty").unwrap();
        a.read_exact(&mut buf[..3]).unwrap();
        assert_eq!(&buf[..3], b"qty");
    }

    #[test]
    fn drop_signals_eof() {
        let (a, mut b) = duplex();
        drop(a);
        let mut buf = [0u8; 1];
        assert_eq!(b.read(&mut buf).unwrap(), 0);
    }
}
