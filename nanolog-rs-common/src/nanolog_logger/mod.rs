mod ring_buf;
mod shareable;

pub struct Logger(shareable::ShareableWriter<ring_buf::RingBuf<4096>>);

impl Logger {
    pub fn write(&mut self, buf: &[u8]) {
        self.0.write(|rb| rb.write(buf));
    }
}

pub struct LogReader(shareable::ShareableReader<ring_buf::RingBuf<4096>>);
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
    let (r, w) = shareable::new_shareable_reader_and_writer(ring_buf::RingBuf::<4096>::new());
    (Logger(w), LogReader(r))
}
