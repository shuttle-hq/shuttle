use std::fs::File;
use std::io::{Read, Write};
use std::os::wasi::prelude::*;

pub fn handle_message(message: &str) -> Option<String> {
    if message == "!hello" {
        Some("world!".to_string())
    } else {
        None
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn __SHUTTLE_EventHandler_message(fd: RawFd) {
    println!("inner handler awoken; interacting with fd={fd}");

    let mut f = unsafe { File::from_raw_fd(fd) };

    let mut buf = Vec::new();
    let mut c_buf = [0; 1];
    loop {
        f.read(&mut c_buf).unwrap();
        if c_buf[0] == 0 {
            break;
        } else {
            buf.push(c_buf[0]);
        }
    }

    let msg = String::from_utf8(buf).unwrap();
    println!("got message: {msg}");

    if let Some(resp) = handle_message(msg.as_str()) {
        f.write_all(resp.as_bytes()).unwrap();
    }
}
