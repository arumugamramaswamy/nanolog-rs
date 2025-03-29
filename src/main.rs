use nanolog_rs_proc_macro::{nanolog, setup_nanolog};
setup_nanolog!();

fn main() {
    let a = 1.1;
    let b = 1;
    let mut s = vec![];
    nanolog!(&mut s, "Hello, world!");
    nanolog!(&mut s, "Hello, world! %f %d", a, b);
    println!("{s:?}")
}
