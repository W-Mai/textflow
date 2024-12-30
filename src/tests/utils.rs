#[macro_export]
macro_rules! assert_line {
    ($text: expr, $line:expr, $expected:expr) => {{
        let res = $line.map(|l| l.slices($text));
        assert_eq!(res, $expected);
    }};
}
