use std::env;

fn main() {
    let filename = &env::args().collect::<Vec<String>>()[1];

    feet::run(filename).expect("Something went wrong!");
}
