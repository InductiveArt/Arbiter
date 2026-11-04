pub fn doubled(value: i32) -> i32 {
    value * 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn doubling_two_gives_four() {
        assert_eq!(super::doubled(2), 4);
    }
}
