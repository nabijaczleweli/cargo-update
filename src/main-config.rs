extern crate cargo_update;

use std::process::exit;


fn main() {
    let result = actual_main().err().unwrap_or(0);
    exit(result);
}

fn actual_main() -> Result<(), i32> {
    let opts = cargo_update::ConfigOptions::parse();

    println!("{:#?}", opts);

    Ok(())
}
