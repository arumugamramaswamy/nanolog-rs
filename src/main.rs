use nanolog_rs_proc_macro::{nanolog, setup_nanolog};
setup_nanolog!();

fn main() {
    let b = 1;
    let a = 1.1;
    let b = 1;
    let c = a;

    let (mut logger, log_reader) = ::nanolog_rs_common::nanolog_logger::create_log_pair();
    // TODO:
    // - timestamping
    // - multiple threads logging at the same time
    //   - setup dedicated logging thread
    //   - send the log_readers back to the logging thread (mpsc channel?) https://doc.rust-lang.org/std/sync/mpsc/index.html
    // Further improvements:
    //   - coalesce writes to stdout / disk

    nanolog!(&mut logger, "Hello, world!");
    nanolog!(&mut logger, "Hello, world!");
    nanolog!(&mut logger, "Hello, world!");
    nanolog!(&mut logger, "Hello, world!");
    nanolog!(&mut logger, "Hello, world! %f %d", a, b);

    let mut log_reader_buf = [0; 4096];

    let n = log_reader.read(&mut log_reader_buf);
    println!("{:?}", &log_reader_buf[..n]);
    nanolog_internal::decode_buf(&log_reader_buf[..n])
}
