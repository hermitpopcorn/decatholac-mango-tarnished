/// Prints to the standard output with a new line,
/// but prepends the current timestamp (in `%Y-%m-%d %H:%M:%S` format) to the message.
#[macro_export]
macro_rules! log {
    ($($arg: tt)*) => {
        println!("{} {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), format!($($arg)*))
    };
}
