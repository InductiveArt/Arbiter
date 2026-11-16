// The files an attempt's folder holds: what the run ordered, the mark that the
// process is ready for its body to begin, the output and the record it left,
// and what it printed.
pub(crate) const ORDER_FILE: &str = "order.json";
pub(crate) const STARTED_FILE: &str = "started";
pub(crate) const OUTPUT_FILE: &str = "output.json";
pub const RECORD_FILE: &str = "record.json";
pub const STDOUT_FILE: &str = "stdout";
pub const STDERR_FILE: &str = "stderr";

// The file holding the input handed to a read, by the read's place among the
// task's reads, for an input no other file holds.
pub(crate) fn input_file(read_at: usize) -> String {
    format!("input-{read_at}.json")
}
