// One tool run comparing a unit's generated code with its contracts.
pub struct Drift {
    pub check: String,
    pub problems: Vec<String>,
    pub seconds: f64,
}
