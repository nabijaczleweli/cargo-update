extern crate cargo_update;

use std::process::exit;


fn main() {
    let result = actual_main();
    exit(result);
}

fn actual_main() -> i32 {
    let opts = cargo_update::Options::parse();

    println!("{:?}", opts);

    0
}
