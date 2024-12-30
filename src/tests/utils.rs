#[macro_export]
macro_rules! assert_line {
    ($text: expr, $line:expr, $expected:expr) => {{
        let res = $line.map(|l| l.slices($text));
        assert_eq!(res, $expected);
    }};
}

#[macro_export]
macro_rules! assert_flow {
    ($text:expr => $max_width:expr => $($expected:literal) +) => {{
        let text = $text;
        let target_texts = &[
            $(
                $expected
            ),+
        ];
        let max_width = $max_width;
        let flow = TextFlow::new(text, max_width);
        for (i, line) in flow.enumerate() {
            assert_eq!(
                line.slices(text),
                target_texts[i],
                "assert failed in line {}",
                i
            );
        }
    }};
}
