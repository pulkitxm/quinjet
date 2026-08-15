/// Terminal geometry is `u16` and collection sizes are `usize`, so every
/// crossing needs a saturating answer rather than a wrapping cast.

pub(crate) fn cells(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

pub(crate) fn count(value: isize) -> usize {
    usize::try_from(value).unwrap_or(0)
}

pub(crate) fn offset(value: u16) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturates_instead_of_wrapping() {
        assert_eq!(cells(usize::MAX), u16::MAX);
        assert_eq!(cells(7), 7);
        assert_eq!(count(-3), 0);
        assert_eq!(count(9), 9);
        assert_eq!(offset(12), 12);
    }
}
