#[macro_export]
macro_rules! take_if {
    ($vector:ident, $pred:expr) => {{
        if $vector.first().map($pred).unwrap_or(false) {
            Some($vector.remove(0))
        } else {
            None
        }
    }};
}
