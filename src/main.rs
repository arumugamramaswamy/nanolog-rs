use nanolog_rs_proc_macro::{nanolog, setup_nanolog};
// TODO: temp for docs
pub use std::alloc;
pub use std::boxed;
use std::cell;
pub use std::sync;
pub use std::time;

pub static LOGGER_SENDER: sync::Mutex<
    cell::OnceCell<sync::mpsc::Sender<::nanolog_rs_common::nanolog_logger::LogReader>>,
> = sync::Mutex::new(cell::OnceCell::new());

setup_nanolog!();

fn main() {
    // startup code
    let start_instant = std::time::Instant::now();
    let (logger_sender, logger_receiver) = std::sync::mpsc::channel();
    {
        let sender = crate::LOGGER_SENDER.lock().unwrap();
        sender.set(logger_sender).unwrap();
    }

    let t1 = std::thread::Builder::new()
        .name("t1".to_string())
        .spawn(|| {
            ::affinity::set_thread_affinity([17]).unwrap();
            let mut logger = setup_logger();

            let a = 1.1;
            let b = 1;

            for x in 0.. {
                nanolog!(&mut logger, "[T1] Hello, world! %f %d", a, x);
                // std::thread::sleep(std::time::Duration::from_nanos(1))
            }
        })
        .unwrap();
    let t2 = std::thread::Builder::new()
        .name("t2".to_string())
        .spawn(|| {
            ::affinity::set_thread_affinity([18]).unwrap();
            let mut logger = setup_logger();

            let a = 1.1;
            let b = 1;

            // TODO:
            // Further improvements:
            //   - coalesce writes to stdout / disk
            //   - api ergonomics (maybe wrapper around thread_spawn that sets up the logger)
            //   - tokio::main equivalent

            for x in 0.. {
                nanolog!(&mut logger, "[T2] Hello, world!");
                nanolog!(&mut logger, "[T2] Hello, world! %f %d", a, x);
                // std::thread::sleep(std::time::Duration::from_nanos(1))
            }
        })
        .unwrap();
    ::affinity::set_thread_affinity([19]).unwrap();

    let mut readers = vec![];
    let mut log_reader_buf = [0; ::nanolog_rs_common::nanolog_logger::RINGBUF_SIZE];

    loop {
        if t1.is_finished() || t2.is_finished() {
            println!("problem!");
            break;
        }
        if let Ok(r) = logger_receiver.try_recv() {
            println!("new!");
            readers.push(r);
        }

        for r in readers.iter() {
            let n = r.read(&mut log_reader_buf);
            // println!("{n}");
            nanolog_internal::decode_buf(&start_instant, &log_reader_buf[..n]);
        }
    }
}

fn setup_logger(
) -> ::nanolog_rs_common::nanolog_logger::Logger<::nanolog_rs_common::nanolog_logger::Spin> {
    let (logger, log_reader) = ::nanolog_rs_common::nanolog_logger::create_log_pair();
    let sender = crate::LOGGER_SENDER.lock().unwrap();
    sender
        .get()
        .expect("log reader channel must be set up at init time")
        .send(log_reader)
        .unwrap();
    // init logger thread
    std::thread::sleep(::std::time::Duration::from_secs(1));
    logger
}
