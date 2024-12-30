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
        let text = "Hello, world!";
        let max_width = 10;
        let flow = TextFlow::new(text, max_width);
        let target_strings = ["Hello, ", "world!"];
        for (i, line) in flow.enumerate() {
            assert_eq!(line.slices(text), target_strings[i], "line {}", i);
        }
    }
}
