// This will fail to compile since it is missing the following section in its Cargo.toml
//
// [lib]
// crate-type = ["cdylib"]
//
fn main() {
    println!("this is not valid as it is not a libray!");
}
