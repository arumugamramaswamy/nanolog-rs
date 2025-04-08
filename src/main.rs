use nanolog_rs_proc_macro::{nanolog, setup_nanolog};
setup_nanolog!();

fn main() {
    let b = 1;
    let a = 1.1;
    let b = 1;
    let c = a;
    let mut s = vec![];
    // TODO: convert the build script to use ::nanolog_rs_common::nanolog_logger::... instead of
    // io_write
    nanolog!(&mut s, "Hello, world!");
    nanolog!(&mut s, "Hello, world! %f %d", a, b);
    println!("{s:?}")
}
