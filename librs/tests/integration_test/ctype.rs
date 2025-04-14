use crate::println;
use bluekernel_test_macro::test;
use librs::ctype::{
    isalnum, isalpha, isascii, isblank, iscntrl, isdigit, isgraph, islower, isprint, ispunct,
    isspace, isupper, isxdigit, toascii, tolower, toupper,
};
type c_int = i32;

pub const EOF: c_int = -1;

// data driven test, each tuple is a test case
const TEST_CASES: &[(
    c_int, // c
    c_int, //isalnum,
    c_int, //isalpha,
    c_int, //isascii,
    c_int, //isblank,
    c_int, //iscntrl,
    c_int, //isdigit,
    c_int, //isgraph,
    c_int, //islower,
    c_int, //isprint,
    c_int, //ispunct,
    c_int, //isspace,
    c_int, //isupper,
    c_int, //isxdigit,
    c_int, //toascii,
    c_int, //tolower,
    c_int, //toupper,
)] = &[
    (
        0x00, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0x00, 0x00,
    ),
    (
        0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, 0xFF, 0xFF,
    ),
    (
        0x1F, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0x1F, 0x1F, 0x1F,
    ),
    (EOF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, EOF, EOF),
];

#[test]
fn test_ctype() {
    for (
        c,
        alnum,
        alpha,
        ascii,
        blank,
        cntrl,
        digit,
        graph,
        lower,
        print,
        punct,
        space,
        upper,
        xdigit,
        xascii,
        xlower,
        xupper,
    ) in TEST_CASES
    {
        assert_eq!(isalnum(*c), *alnum);
        assert_eq!(isalpha(*c), *alpha);
        assert_eq!(isascii(*c), *ascii);
        assert_eq!(isblank(*c), *blank);
        assert_eq!(iscntrl(*c), *cntrl);
        assert_eq!(isdigit(*c), *digit);
        assert_eq!(isgraph(*c), *graph);
        assert_eq!(islower(*c), *lower);
        assert_eq!(isprint(*c), *print);
        assert_eq!(ispunct(*c), *punct);
        assert_eq!(isspace(*c), *space);
        assert_eq!(isupper(*c), *upper);
        assert_eq!(isxdigit(*c), *xdigit);
        assert_eq!(toascii(*c), *xascii);
        assert_eq!(tolower(*c), *xlower);
        assert_eq!(toupper(*c), *xupper);
    }
}
