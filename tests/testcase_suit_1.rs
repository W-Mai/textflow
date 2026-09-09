use textflow::TextFlow;

fn assert_cases(cases: &[(&str, &str, usize, &[&str])]) {
    for (name, text, width, expected) in cases {
        let actual = TextFlow::new(text, *width)
            .map(|line| line.text())
            .collect::<Vec<_>>();
        assert_eq!(actual, *expected, "{name}: {text:?} at width {width}");
    }
}

#[test]
fn wraps_latin_words_and_whitespace() {
    assert_cases(&[
        ("basic words", "Hello, world!", 10, &["Hello,", "world!"]),
        (
            "emergency word breaks",
            "Hello, world!",
            3,
            &["Hel", "lo,", "wor", "ld!"],
        ),
        (
            "word opportunities",
            "The quick brown fox jumps over a lazy dog.",
            15,
            &["The quick brown", "fox jumps over", "a lazy dog."],
        ),
        (
            "repeated spaces",
            "abc, bcd, efg  bc",
            5,
            &["abc,", "bcd,", "efg", "bc"],
        ),
    ]);
}

#[test]
fn wraps_wide_and_mixed_text() {
    assert_cases(&[
        ("wide characters", "你好中国", 2, &["你", "好", "中", "国"]),
        (
            "wide punctuation tail",
            "This is a Text》〉>?!",
            20,
            &["This is a", "Text》〉>?!"],
        ),
        (
            "mixed scripts",
            "八百标兵奔北坡炮兵并排北边跑666中英文测试。The quick brown fox jumps over a lazy dog. abcdefghijklmnopq rstuvwxyz",
            14,
            &[
                "八百标兵奔北坡",
                "炮兵并排北边跑",
                "666中英文测",
                "试。The quick",
                "brown fox",
                "jumps over a",
                "lazy dog.",
                "abcdefghijklmn",
                "opq rstuvwxyz",
            ],
        ),
        (
            "mandatory break",
            "为了提供更好的服务和服务。\n请您在使用前充分阅读《TextFlowwwwwwwwwwwwwwwwww 使用隐私 Policy》",
            25,
            &[
                "为了提供更好的服务和服",
                "务。",
                "请您在使用前充分阅读",
                "《TextFlowwwwwwwwwwwwwwww",
                "ww 使用隐私 Policy》",
            ],
        ),
        (
            "mixed identifier",
            "实时操作系统 Nuttx》。",
            20,
            &["实时操作系统", "Nuttx》。"],
        ),
    ]);
}

#[test]
fn keeps_punctuation_groups_readable() {
    assert_cases(&[
        (
            "opening group",
            "<〈《Teext a>>>",
            12,
            &["<〈《Teext", "a>>>"],
        ),
        (
            "nested group",
            "<〈《Tee<ext><>>",
            12,
            &["<〈《Tee", "<ext><>>"],
        ),
        (
            "nested long group",
            "<〈《Tee<eext><>>",
            12,
            &["<〈《Tee", "<eext><>>"],
        ),
        (
            "wide closing group",
            "<〈<<《你》>",
            10,
            &["<〈<<《你", "》>"],
        ),
        (
            "full-width divider",
            "<〈<<《Loooooo｜ong>>",
            14,
            &["<〈<<《Loooooo", "｜ong>>"],
        ),
        (
            "quoted phrase",
            "this is aaaa \"text word\" test",
            15,
            &["this is aaaa", "\"text word\"", "test"],
        ),
        (
            "quoted split",
            "this is a \"text word\" test",
            15,
            &["this is a \"text", "word\" test"],
        ),
        (
            "angle group",
            "this is a <text> test",
            15,
            &["this is a", "<text> test"],
        ),
        (
            "hyphen break",
            "this is a text-test",
            15,
            &["this is a text-", "test"],
        ),
        (
            "long bracketed word",
            "《Loooooooooooooooong Text》",
            20,
            &["《Loooooooooooooooon", "g Text》"],
        ),
        (
            "nested bracket expression",
            "a book named 《<《「Wow》>」",
            27,
            &["a book named", "《<《「Wow》>」"],
        ),
        (
            "open quote tail",
            "a book named \"various",
            13,
            &["a book named", "\"various"],
        ),
    ]);
}

#[test]
fn advances_at_minimum_width() {
    assert_cases(&[(
        "one-column words",
        "an    apple         \"is\" a fruit",
        1,
        &[
            "a", "n", "a", "p", "p", "l", "e", "\"", "i", "s", "\"", "a", "f", "r", "u", "i", "t",
        ],
    )]);
}
