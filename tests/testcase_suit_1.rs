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
    }
}
