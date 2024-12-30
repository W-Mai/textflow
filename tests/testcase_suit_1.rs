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
        assert_flow!(
            "你好中国"
            => 2 =>
            "你"
            "好"
            "中"
            "国"
        );
    }

    #[test]
    fn test_4() {
        assert_flow!(
            "Hello, world!"
            => 10 =>
            "Hello, "
            "world!"
        );
    }
    #[test]
    fn test_5() {
        assert_flow!(
            "Hello, world!"
            => 3 =>
            "Hel"
            "lo,"
            "wor"
            "ld!"
        );
    }

    #[test]
    fn test_6() {
        assert_flow!(
            "This is a Text》〉>?!"
            => 20 =>
            "This is a "
            "Text》〉>?!"
        );
    }

    #[test]
    fn test_7() {
        assert_flow!(
            "<〈《Teext a>>>"
            => 12 =>
            "<〈《Teext "
            "a>>>"
        );
    }

    #[test]
    fn test_8() {
        assert_flow!(
            "<〈《Tee<ext><>>"
            => 12 =>
            "<〈《Tee"
            "<ext><>>"
        );
    }

    #[test]
    fn test_9() {
        assert_flow!(
            "<〈《Tee<eext><>>"
            => 12 =>
            "<〈《Tee"
            "<eext><>>"
        );
    }

    #[test]
    fn test_10() {
        assert_flow!(
            "<〈<<《你》>"
            => 10 =>
            "<〈<<《你"
            "》>"
        );
    }

    #[test]
    fn test_11() {
        assert_flow!(
            "<〈<<《Loooooo｜ong>>"
            => 14 =>
            "<〈<<《Loooooo"
            "｜ong>>"
        );
    }

    #[test]
    fn test_12() {
        assert_flow!(
            "this is aaaa \"text word\" test"
            => 15 =>
            "this is aaaa "
            "\"text word\" "
            "test"
        );
    }

    #[test]
    fn test_13() {
        assert_flow!(
            "this is a \"text word\" test"
            => 15 =>
            "this is a \"text"
            "word\" test"
        );
    }

    #[test]
    fn test_14() {
        assert_flow!(
            "this is a <text> test"
            => 15 =>
            "this is a "
            "<text> test"
        );
    }

    #[test]
    fn test_15() {
        assert_flow!(
            "this is a text-test"
            => 15 =>
            "this is a text-"
            "test"
        );
    }

    #[test]
    fn test_16() {
        assert_flow!(
            "实时操作系统 Nuttx》。"
            => 20 =>
            "实时操作系统 "
            "Nuttx》。"
        );
    }
}
