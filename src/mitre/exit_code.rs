// Codes 126 & 127 are traditionally reserved for shells
// so we'll use higher codes. Windows and POSIX both support
// exit statuses i32. (unix must use waitid() not wait())
//
// - https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_08_02

#[no_mangle]
pub static CONFIG_PROBLEM: i32 = 150;

#[no_mangle]
pub static MIGRATION_DIR_PROBLEM: i32 = 151;

#[no_mangle]
pub static NO_CONFIG_SPECIFIED: i32 = 152;

#[no_mangle]
pub static STATE_STORE_PROBLEM: i32 = 153;
