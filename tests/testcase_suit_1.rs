#[cfg(test)]
mod testcase_suit_1 {
    use textflow::TextFlow;

    macro_rules! assert_line {
        ($text: expr, $line:expr, $expected:expr) => {{
            let res = $line.map(|l| l.slices($text));
            assert_eq!(res, $expected);
        }};
    }

    #[test]
    fn test_1() {
        let text = "Hello, world!";
        let max_width = 10;
        let mut flow = TextFlow::new(text, max_width);

        let line = flow.next();
        println!("{:#?}", line);
        let line = flow.next();
        println!("{:#?}", line);
    }

    #[test]
    fn test_2() {
        let text = "Hello, world!";
        let max_width = 10;
        let mut flow = TextFlow::new(text, max_width);

        assert_line!(text, flow.next(), Some("Hello, "));
        assert_line!(text, flow.next(), Some("world!"));
        assert_line!(text, flow.next(), None);
        assert_line!(text, flow.next(), None);
    }
}
