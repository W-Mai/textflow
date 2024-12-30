#[macro_use]
extern crate textflow;

#[cfg(test)]
mod testcase_suit_1 {
    use textflow::TextFlow;

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

    #[test]
    fn test_3() {
        assert_flow!("Hello, world!", ["Hello, ", "world!"], 10);
        assert_flow!("Hello, world!", ["Hel", "lo,", "wor", "ld!"], 3);
    }
}
