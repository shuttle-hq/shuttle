use crate::args::args;

args! {
    pub struct NextArgs {
        "--port" => pub port: u16,
    }
}
