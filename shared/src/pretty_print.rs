use std::fmt::Display;

pub const RESET: &str = "\x1B[0m";
pub const RED: &str = "\x1B[31m";
pub const GREEN: &str = "\x1B[32m";
pub const YELLOW: &str = "\x1B[33m";
pub const ORANGE: &str = "\x1B[38;5;208m";
pub const LIGHT_GRAY: &str = "\x1B[90m";

#[inline(always)]
pub fn print_success(msg: impl Display) {
    println!("{GREEN}[+]{RESET} {msg}");
}

#[inline(always)]
pub fn print_info(msg: impl Display) {
    println!("{YELLOW}[i]{RESET} {msg}");
}

#[inline(always)]
pub fn print_failed(msg: impl Display) {
    println!("{RED}[-]{RESET} {msg}");
}
