pub struct RingBuffer {
    buf: Vec<u8>,
    cap: usize,
}

impl RingBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            cap,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        if bytes.len() >= self.cap {
            self.buf.clear();
            self.buf.extend_from_slice(&bytes[bytes.len() - self.cap..]);
            return;
        }
        let new_total = self.buf.len() + bytes.len();
        if new_total > self.cap {
            let drop_n = new_total - self.cap;
            self.buf.drain(..drop_n);
        }
        self.buf.extend_from_slice(bytes);
    }

    pub fn as_string(&self) -> String {
        String::from_utf8_lossy(&self.buf).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_pushes_accumulate() {
        let mut rb = RingBuffer::new(10);
        rb.push(b"abc");
        rb.push(b"de");
        assert_eq!(rb.as_string(), "abcde");
    }

    #[test]
    fn overflow_keeps_tail() {
        let mut rb = RingBuffer::new(5);
        rb.push(b"abcdefgh");
        assert_eq!(rb.as_string(), "defgh");
    }

    #[test]
    fn incremental_overflow_keeps_tail() {
        let mut rb = RingBuffer::new(5);
        rb.push(b"abc");
        rb.push(b"de");
        rb.push(b"fg");
        assert_eq!(rb.as_string(), "cdefg");
    }

    #[test]
    fn exact_cap_push() {
        let mut rb = RingBuffer::new(4);
        rb.push(b"abcd");
        assert_eq!(rb.as_string(), "abcd");
    }
}
