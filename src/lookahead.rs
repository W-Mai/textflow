pub(crate) struct Lookahead<I>
where
    I: Iterator,
{
    iterator: I,
    front: Option<I::Item>,
    ahead: Option<I::Item>,
    advanced: bool,
}

impl<I> Lookahead<I>
where
    I: Iterator,
{
    pub(crate) fn new(iterator: I) -> Self {
        Self {
            iterator,
            front: None,
            ahead: None,
            advanced: false,
        }
    }

    pub(crate) fn peek(&mut self) -> Option<&I::Item> {
        if self.front.is_none() {
            self.front = self.iterator.next();
        }
        if !self.advanced {
            return self.front.as_ref();
        }
        if self.front.is_some() && self.ahead.is_none() {
            self.ahead = self.iterator.next();
        }
        self.ahead.as_ref()
    }

    pub(crate) fn advance_cursor(&mut self) -> &mut Self {
        debug_assert!(!self.advanced);
        self.advanced = true;
        self
    }
}

impl<I> Iterator for Lookahead<I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front.is_none() {
            self.front = self.iterator.next();
        }
        let item = self.front.take();
        self.front = self.ahead.take();
        self.advanced = false;
        item
    }
}
