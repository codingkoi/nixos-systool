#[macro_export]
macro_rules! info {
    ( $msg:expr ) => {
        println!("{}", $msg.italic());
    };
}

#[macro_export]
macro_rules! error {
    ( $msg:expr ) => {
        eprintln!("{}", $msg.red().italic());
    };
}
