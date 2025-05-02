struct IsPowerOf2<const N: usize>;
use std::cell::Cell;
use std::sync::atomic;

impl<const N: usize> IsPowerOf2<N> {
    const OK: () = assert!(N & (N - 1) == 0);
}

pub trait WaitStrategy {
    fn wait_to_write<const N: usize>(rb: &mut RingBuf<N>, buf: &[u8]) {}
}

pub struct RingBuf<const N: usize> {
    buf: [u8; N],
    head: atomic::AtomicUsize, // where the reader is reading from
    tail: atomic::AtomicUsize, // where the writer is writing to
    writer_head: usize,        // best conservative guess
    writer_tail: usize,        // known
    reader_head: Cell<usize>,  // known
}

impl<const N: usize> RingBuf<N> {
    pub fn boxed_unsafe_cell_new() -> Box<std::cell::UnsafeCell<Self>> {
        unsafe {
            let layout = std::alloc::Layout::new::<std::cell::UnsafeCell<Self>>();
            let ptr = std::alloc::alloc(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            let rb = (&mut *(ptr as *mut std::cell::UnsafeCell<Self>)).get_mut();
            rb.head = atomic::AtomicUsize::new(0);
            rb.tail = atomic::AtomicUsize::new(0);
            rb.reader_head = 0.into();
            rb.writer_head = 0;
            rb.writer_tail = 0;
            Box::from_raw(ptr as *mut std::cell::UnsafeCell<Self>)
        }
    }
    pub fn new() -> Self {
        let _ = IsPowerOf2::<N>::OK;
        Self {
            buf: [0; N],
            head: atomic::AtomicUsize::new(0),
            tail: atomic::AtomicUsize::new(0),
            reader_head: 0.into(),
            writer_head: 0,
            writer_tail: 0,
        }
    }

    // eventually impl 0 copy direct uses of the underlying buffer
    pub fn read_all(&self, buf: &mut [u8]) -> usize {
        // extract head from inside the cell (required for interior mutability)
        let mut head_cell = Cell::new(0);
        self.reader_head.swap(&head_cell);
        let head = head_cell.get_mut();

        let tail = self.tail.load(atomic::Ordering::Acquire); // other thread writes this value
                                                              // so I need to load it as acquire

        let n = tail - *head;
        if n == 0 {
            self.reader_head.swap(&head_cell);
            return 0;
        }

        let start = *head % N;
        let end = tail % N;

        if start < end {
            buf[..n].copy_from_slice(&self.buf[start..end]);
        } else {
            let n_temp = N - start;
            buf[..n_temp].copy_from_slice(&self.buf[start..]);
            buf[n_temp..n].copy_from_slice(&self.buf[..end]);
        }

        // consumed all the way upto tail, so we can just store it directly
        // update the cell with the latest reader_head
        *head = tail;
        self.reader_head.swap(&head_cell);
        self.head.store(tail, atomic::Ordering::Release);
        n
    }

    pub fn write<W: WaitStrategy>(&mut self, buf: &[u8]) {
        if buf.len() == 0 {
            return;
        }
        W::wait_to_write(self, buf);

        let wrapped_tail = self.writer_tail % N;

        if wrapped_tail + buf.len() <= N {
            self.buf[wrapped_tail..(wrapped_tail + buf.len())].copy_from_slice(buf);
        } else {
            let n_temp = N - wrapped_tail;
            self.buf[wrapped_tail..].copy_from_slice(&buf[..n_temp]);
            self.buf[..(buf.len() - n_temp)].copy_from_slice(&buf[n_temp..buf.len()]);
        }

        self.writer_tail += buf.len();
    }

    pub fn commit_write(&mut self) {
        self.tail.store(self.writer_tail, atomic::Ordering::Release);
    }
}

pub struct Panic;

impl WaitStrategy for Panic {
    fn wait_to_write<const N: usize>(rb: &mut RingBuf<N>, buf: &[u8]) {
        let head = rb.head.load(atomic::Ordering::Acquire);
        let n = rb.writer_tail - head;
        let remaining = N - n;

        if buf.len() > remaining {
            panic!(
                "too much to write {} - {} = {}. remaining = {}. buf_len = {}",
                rb.writer_tail,
                head,
                n,
                remaining,
                buf.len()
            );
        }
    }
}

pub struct Spin;

impl WaitStrategy for Spin {
    fn wait_to_write<const N: usize>(rb: &mut RingBuf<N>, buf: &[u8]) {
        loop {
            let n = rb.writer_tail - rb.writer_head;
            let remaining = N - n;

            if buf.len() <= remaining {
                break;
            }
            rb.writer_head = rb.head.load(atomic::Ordering::Acquire);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_write_read() {
        let mut r = RingBuf::<16>::new();
        let a = [1, 2, 3];
        r.write::<Panic>(&a);
        r.commit_write();
        let mut v = vec![0; 16];
        let n = r.read_all(&mut v);
        assert_eq!(n, a.len());
        assert_eq!(&v[..n], &a);

        let a = [1; 16];
        r.write::<Panic>(&a);
        r.commit_write();
        let mut v = vec![0; 16];
        let n = r.read_all(&mut v);
        assert_eq!(n, a.len());
        assert_eq!(&v[..n], &a);
    }

    #[test]
    fn test_empty_buffer() {
        let r = RingBuf::<16>::new();
        let mut v = vec![0; 16];
        let n = r.read_all(&mut v);
        assert_eq!(n, 0);
    }

    // #[test]
    // fn test_partial_read() {
    //     let mut r = RingBuf::<16>::new();
    //     let data = [1, 2, 3, 4, 5];
    //     r.write::<Panic>(&data);

    //     // Read only part of the data
    //     let mut v = vec![0; 3];
    //     let n = r.read_all(&mut v);
    //     assert_eq!(n, 3);
    //     assert_eq!(&v[..n], &data[..3]);
    // }

    #[test]
    fn test_multiple_writes_single_read() {
        let mut r = RingBuf::<16>::new();
        r.write::<Panic>(&[1, 2, 3]);
        r.write::<Panic>(&[4, 5]);
        r.write::<Panic>(&[6, 7, 8, 9]);
        r.commit_write();

        let mut v = vec![0; 16];
        let n = r.read_all(&mut v);
        assert_eq!(n, 9);
        assert_eq!(&v[..n], &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_write_read_cycles() {
        let mut r = RingBuf::<16>::new();

        // Write some data
        r.write::<Panic>(&[1, 2, 3]);
        r.commit_write();

        // Read it out
        let mut v = vec![0; 3];
        let n = r.read_all(&mut v);
        assert_eq!(n, 3);
        assert_eq!(&v[..n], &[1, 2, 3]);

        // Write more data
        r.write::<Panic>(&[4, 5, 6, 7]);
        r.commit_write();

        // Read it out
        let mut v = vec![0; 4];
        let n = r.read_all(&mut v);
        assert_eq!(n, 4);
        assert_eq!(&v[..n], &[4, 5, 6, 7]);

        // Verify buffer is empty
        let mut v = vec![0; 1];
        let n = r.read_all(&mut v);
        assert_eq!(n, 0);
    }

    #[test]
    fn test_wrap_around_write() {
        let mut r = RingBuf::<8>::new();

        // Fill most of the buffer
        r.write::<Panic>(&[1, 2, 3, 4, 5]);
        r.commit_write();

        // Read it out
        let mut v = vec![0; 5];
        let n = r.read_all(&mut v);
        assert_eq!(n, 5);

        // Now write data that will wrap around
        r.write::<Panic>(&[10, 11, 12, 13, 14, 15]);
        r.commit_write();

        // Read it out
        let mut v = vec![0; 6];
        let n = r.read_all(&mut v);
        assert_eq!(n, 6);
        assert_eq!(&v[..n], &[10, 11, 12, 13, 14, 15]);
    }

    // #[test]
    // fn test_wrap_around_read() {
    //     let mut r = RingBuf::<8>::new();

    //     // Fill buffer partially
    //     r.write::<Panic>(&[1, 2, 3, 4, 5]);

    //     // Read part of it
    //     let mut v = vec![0; 2];
    //     let n = r.read_all(&mut v);
    //     assert_eq!(n, 2);
    //     assert_eq!(&v[..n], &[1, 2]);

    //     // Write more so it wraps around
    //     r.write::<Panic>(&[6, 7, 8, 9, 10]);

    //     // Read everything
    //     let mut v = vec![0; 8];
    //     let n = r.read_all(&mut v);
    //     assert_eq!(n, 8);
    //     assert_eq!(&v[..n], &[3, 4, 5, 6, 7, 8, 9, 10]);
    // }

    #[test]
    fn test_full_buffer() {
        let mut r = RingBuf::<4>::new();

        // Fill the buffer completely
        r.write::<Panic>(&[1, 2, 3, 4]);
        r.commit_write();

        // Try to read it all
        let mut v = vec![0; 4];
        let n = r.read_all(&mut v);
        assert_eq!(n, 4);
        assert_eq!(&v[..n], &[1, 2, 3, 4]);

        // Fill it again
        r.write::<Panic>(&[5, 6, 7, 8]);
        r.commit_write();

        // Read it all again
        let mut v = vec![0; 4];
        let n = r.read_all(&mut v);
        assert_eq!(n, 4);
        assert_eq!(&v[..n], &[5, 6, 7, 8]);
    }

    #[test]
    #[should_panic(expected = "too much to write")]
    fn test_overflow() {
        let mut r = RingBuf::<4>::new();

        // Write 3 bytes
        r.write::<Panic>(&[1, 2, 3]);

        // Try to write 2 more bytes, which exceeds capacity
        r.write::<Panic>(&[4, 5]);
    }

    #[test]
    fn test_different_buffer_sizes() {
        // Test with a larger buffer size
        let mut r = RingBuf::<32>::new();
        let data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        r.write::<Panic>(&data);
        r.commit_write();

        let mut v = vec![0; 10];
        let n = r.read_all(&mut v);
        assert_eq!(n, 10);
        assert_eq!(&v[..n], &data[..]);

        // Test with a smaller buffer size
        let mut r = RingBuf::<4>::new();
        let data = [0, 1, 2, 3];
        r.write::<Panic>(&data);
        r.commit_write();

        let mut v = vec![0; 4];
        let n = r.read_all(&mut v);
        assert_eq!(n, 4);
        assert_eq!(&v[..n], &data[..]);
    }

    // #[test]
    // fn test_read_buffer_smaller_than_data() {
    //     let mut r = RingBuf::<16>::new();
    //     let data = [1, 2, 3, 4, 5, 6, 7, 8];
    //     r.write::<Panic>(&data);

    //     // Read buffer is smaller than written data
    //     let mut v = vec![0; 4];
    //     let n = r.read_all(&mut v);

    //     // Should read only up to the size of the read buffer
    //     assert_eq!(n, 4);
    //     assert_eq!(&v[..n], &data[..4]);

    //     // Read the rest
    //     let mut v = vec![0; 4];
    //     let n = r.read_all(&mut v);
    //     assert_eq!(n, 4);
    //     assert_eq!(&v[..n], &data[4..]);
    // }

    // #[test]
    // fn test_sequential_writes_wrap_around() {
    //     let mut r = RingBuf::<8>::new();

    //     // Fill 6 bytes
    //     r.write::<Panic>(&[1, 2, 3, 4, 5, 6]);

    //     // Read 4 bytes
    //     let mut v = vec![0; 4];
    //     let n = r.read_all(&mut v);
    //     assert_eq!(n, 4);
    //     assert_eq!(&v[..n], &[1, 2, 3, 4]);

    //     // Write 6 more bytes (should wrap around)
    //     r.write::<Panic>(&[7, 8, 9, 10, 11, 12]);

    //     // Read all
    //     let mut v = vec![0; 8];
    //     let n = r.read_all(&mut v);
    //     assert_eq!(n, 8);
    //     assert_eq!(&v[..n], &[5, 6, 7, 8, 9, 10, 11, 12]);
    // }

    #[test]
    fn test_power_of_2_sizes() {
        // These should all work
        let _r1 = RingBuf::<1>::new();
        let _r2 = RingBuf::<2>::new();
        let _r4 = RingBuf::<4>::new();
        let _r8 = RingBuf::<8>::new();
        let _r16 = RingBuf::<16>::new();
        let _r32 = RingBuf::<32>::new();
        let _r64 = RingBuf::<64>::new();
        let _r128 = RingBuf::<128>::new();
        let _r256 = RingBuf::<256>::new();
        let _r512 = RingBuf::<512>::new();
        let _r1024 = RingBuf::<1024>::new();

        // Note: We don't test non-power-of-2 sizes as they would fail at compile time
        // due to the IsPowerOf2 check in the new() method
    }
}
