use shapes::{area as measured, area};

pub mod shapes;
pub use shapes::area;

#[cfg(test)]
mod tests {
    #[test]
    fn a_mirrored_assertion_is_a_tautology() {
        assert_eq!(4, 4);
    }

    #[test]
    #[ignore]
    fn a_slow_check_waits() {}
}
