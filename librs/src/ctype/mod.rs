use crate::types::c_int;
#[no_mangle]
pub extern "C" fn isalnum(c: c_int) -> c_int {
    c_int::from(isdigit(c) != 0 || isalpha(c) != 0)
}

#[no_mangle]
pub extern "C" fn isalpha(c: c_int) -> c_int {
    c_int::from(islower(c) != 0 || isupper(c) != 0)
}

#[deprecated]
#[no_mangle]
pub extern "C" fn isascii(c: c_int) -> c_int {
    c_int::from((c & !0x7f) == 0)
}

#[no_mangle]
pub extern "C" fn isblank(c: c_int) -> c_int {
    c_int::from(c == c_int::from(b' ') || c == c_int::from(b'\t'))
}

#[no_mangle]
pub extern "C" fn iscntrl(c: c_int) -> c_int {
    c_int::from((c >= 0x00 && c <= 0x1f) || c == 0x7f)
}

#[no_mangle]
pub extern "C" fn isdigit(c: c_int) -> c_int {
    c_int::from(c >= c_int::from(b'0') && c <= c_int::from(b'9'))
}

#[no_mangle]
pub extern "C" fn isgraph(c: c_int) -> c_int {
    c_int::from(c >= 0x21 && c <= 0x7e)
}

#[no_mangle]
pub extern "C" fn islower(c: c_int) -> c_int {
    c_int::from(c >= c_int::from(b'a') && c <= c_int::from(b'z'))
}

#[no_mangle]
pub extern "C" fn isprint(c: c_int) -> c_int {
    c_int::from(c >= 0x20 && c < 0x7f)
}

#[no_mangle]
pub extern "C" fn ispunct(c: c_int) -> c_int {
    c_int::from(
        (c >= c_int::from(b'!') && c <= c_int::from(b'/'))
            || (c >= c_int::from(b':') && c <= c_int::from(b'@'))
            || (c >= c_int::from(b'[') && c <= c_int::from(b'`'))
            || (c >= c_int::from(b'{') && c <= c_int::from(b'~')),
    )
}

#[no_mangle]
pub extern "C" fn isspace(c: c_int) -> c_int {
    c_int::from(
        c == c_int::from(b' ')
            || c == c_int::from(b'\t')
            || c == c_int::from(b'\n')
            || c == c_int::from(b'\r')
            || c == 0x0b
            || c == 0x0c,
    )
}

#[no_mangle]
pub extern "C" fn isupper(c: c_int) -> c_int {
    c_int::from(c >= c_int::from(b'A') && c <= c_int::from(b'Z'))
}

#[no_mangle]
pub extern "C" fn isxdigit(c: c_int) -> c_int {
    c_int::from(isdigit(c) != 0 || (c | 32 >= c_int::from(b'a') && c | 32 <= c_int::from(b'f')))
}

#[deprecated]
#[no_mangle]
pub extern "C" fn toascii(c: c_int) -> c_int {
    c & 0x7f
}

#[no_mangle]
pub extern "C" fn tolower(c: c_int) -> c_int {
    if isupper(c) != 0 {
        c | 0x20
    } else {
        c
    }
}

#[no_mangle]
pub extern "C" fn toupper(c: c_int) -> c_int {
    if islower(c) != 0 {
        c & !0x20
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test_case]
    fn check_is_alnum() {
        assert_eq!(isalnum(b'0' as c_int), 1);
        assert_eq!(isalnum(b'9' as c_int), 1);
        assert_eq!(isalnum(b'a' as c_int), 1);
        assert_eq!(isalnum(b'z' as c_int), 1);
        assert_eq!(isalnum(b'A' as c_int), 1);
        assert_eq!(isalnum(b'Z' as c_int), 1);
        assert_eq!(isalnum(b' ' as c_int), 0);
        assert_eq!(isalnum(b'!' as c_int), 0);
    }

    #[test_case]
    fn check_is_alpha() {
        assert_eq!(isalpha(b'a' as c_int), 1);
        assert_eq!(isalpha(b'z' as c_int), 1);
        assert_eq!(isalpha(b'A' as c_int), 1);
        assert_eq!(isalpha(b'Z' as c_int), 1);
        assert_eq!(isalpha(b'0' as c_int), 0);
        assert_eq!(isalpha(b'9' as c_int), 0);
        assert_eq!(isalpha(b' ' as c_int), 0);
        assert_eq!(isalpha(b'!' as c_int), 0);
    }

    #[test_case]
    fn check_is_ascii() {
        assert_eq!(isascii(b'a' as c_int), 1);
        assert_eq!(isascii(0x80), 0);
    }

    #[test_case]
    fn check_is_blank() {
        assert_eq!(isblank(b' ' as c_int), 1);
        assert_eq!(isblank(b'\t' as c_int), 1);
        assert_eq!(isblank(b'a' as c_int), 0);
        assert_eq!(isblank(b'0' as c_int), 0);
    }

    #[test_case]
    fn check_is_cntrl() {
        assert_eq!(iscntrl(0x00), 1);
        assert_eq!(iscntrl(0x1f), 1);
        assert_eq!(iscntrl(0x7f), 1);
        assert_eq!(iscntrl(0x20), 0);
        assert_eq!(iscntrl(0x7e), 0);
    }

    #[test_case]
    fn check_is_digit() {
        assert_eq!(isdigit(b'0' as c_int), 1);
        assert_eq!(isdigit(b'9' as c_int), 1);
        assert_eq!(isdigit(b'a' as c_int), 0);
        assert_eq!(isdigit(b'z' as c_int), 0);
    }

    #[test_case]
    fn check_is_space() {
        assert_eq!(isspace(b' ' as c_int), 1);
        assert_eq!(isspace(b'\t' as c_int), 1);
        assert_eq!(isspace(b'\n' as c_int), 1);
        assert_eq!(isspace(b'\r' as c_int), 1);
    }
}
