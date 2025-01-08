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

        assert_line!(text, flow.next(), Some("Hello,"));
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
            "Hello,"
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
            "This is a"
            "Text》〉>?!"
        );
    }

    #[test]
    fn test_7() {
        assert_flow!(
            "<〈《Teext a>>>"
            => 12 =>
            "<〈《Teext"
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
            "this is aaaa"
            "\"text word\""
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
            "this is a"
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
            "实时操作系统"
            "Nuttx》。"
        );
    }

    #[test]
    fn test_17() {
        assert_flow!(
            "The quick brown fox jumps over a lazy dog."
            => 15 =>
            "The quick brown"
            "fox jumps over"
            "a lazy dog."
        );
    }

    #[test]
    fn test_18() {
        assert_flow!(
            "八百标兵奔北坡炮兵并排北边跑666中英文测试。The quick brown fox jumps over a lazy dog. abcdefghijklmnopq rstuvwxyz"
            => 14 =>
            "八百标兵奔北坡"
            "炮兵并排北边跑"
            "666中英文测"
            "试。The quick"
            "brown fox"
            "jumps over a"
            "lazy dog."
            "abcdefghijklmn"
            "opq rstuvwxyz"
        )
    }

    #[test]
    fn test_19() {
        assert_flow!(
            "为了提供更好的服务和服务。\n请您在使用前充分阅读《TextFlowwwwwwwwwwwwwwwwww 使用隐私 Policy》"
            => 25 =>
            "为了提供更好的服务和服"
            "务。"
            "请您在使用前充分阅读"
            "《TextFlowwwwwwwwwwwwwwww"
            "ww 使用隐私 Policy》"
        )
    }

    #[test]
    fn test_20() {
        assert_flow!(
            "《Loooooooooooooooong Text》"
            => 20 =>
            "《Loooooooooooooooon"
            "g Text》"
        );
    }

    #[test]
    fn test_21() {
        assert_flow!(
            "abc, bcd, efg  bc"
            => 5 =>
            "abc,"
            "bcd,"
            "efg"
            "bc"
        );
    }

    #[test]
    fn test_22() {
        assert_flow!(
            "an    apple         \"is\" a fruit"
            => 1 =>
            "a"
            "n"
            "a"
            "p"
            "p"
            "l"
            "e"
            "\""
            "i"
            "s"
            "\""
            "a"
            "f"
            "r"
            "u"
            "i"
            "t"
        );
    }

    #[test]
    fn test_23() {
        assert_flow!(
            "a book named 《<《「Wow》>」"
            => 27 =>
            "a book named"
            "《<《「Wow》>」"
        );
    }

    #[test]
    fn test_24() {
        assert_flow!(
            "a book named \"various"
            => 13 =>
            "a book named"
            "\"various"
        );
    }
}
