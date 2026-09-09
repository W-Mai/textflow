pub(crate) struct SliceWriter<'a, T> {
    output: &'a mut [T],
    required: usize,
}

impl<'a, T> SliceWriter<'a, T> {
    pub(crate) const fn new(output: &'a mut [T]) -> Self {
        Self {
            output,
            required: 0,
        }
    }

    pub(crate) fn push(&mut self, value: T) {
        if let Some(slot) = self.output.get_mut(self.required) {
            *slot = value;
        }
        self.required += 1;
    }

    pub(crate) fn last_mut(&mut self) -> Option<&mut T> {
        self.required
            .checked_sub(1)
            .and_then(|index| self.output.get_mut(index))
    }

    pub(crate) fn finish(self) -> Result<&'a [T], usize> {
        if self.required > self.output.len() {
            Err(self.required)
        } else {
            Ok(&self.output[..self.required])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_exact_required_capacity_after_overflow() {
        let mut output = [0; 1];
        let mut writer = SliceWriter::new(&mut output);
        writer.push(1);
        writer.push(2);

        assert_eq!(writer.finish(), Err(2));
        assert_eq!(output, [1]);
    }
}
