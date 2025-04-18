mod ring_buf;
mod shareable;

pub const RINGBUF_SIZE: usize = 512 * 1024;
#[derive(Clone)]
pub struct Logger(shareable::ShareableWriter<ring_buf::RingBuf<RINGBUF_SIZE>>);

impl Logger {
    pub fn write(&mut self, buf: &[u8]) {
        self.0.write(|rb| rb.write(buf));
    }

    pub fn commit_write(&mut self) {
        self.0.write(|rb| rb.commit_write())
    }
}

pub struct LogReader(shareable::ShareableReader<ring_buf::RingBuf<RINGBUF_SIZE>>);
impl LogReader {
    pub fn read(&self, buf: &mut [u8]) -> usize {
        // TODO: see if this causes any problems
        let mut a = 0;
        self.0.read(|rb| {
            a = rb.read_all(buf);
        });
        a
    }
}

pub fn create_log_pair() -> (Logger, LogReader) {
    let (r, w) =
        shareable::new_shareable_reader_and_writer(ring_buf::RingBuf::<RINGBUF_SIZE>::new());
    (Logger(w), LogReader(r))
}
