// SPDX-License-Identifier: GPL-3.0-or-later
#[macro_export]
macro_rules! info {
    ( $msg:expr ) => {
        println!("{}", $msg.italic())
    };
}

#[macro_export]
macro_rules! warn {
    ( $msg:expr ) => {
        println!("{}", $msg.yellow())
    };
}

#[macro_export]
macro_rules! error {
    ( $msg:expr ) => {
        eprintln!("{}", $msg.red().bold())
    };
}
