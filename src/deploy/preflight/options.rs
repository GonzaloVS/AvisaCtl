pub struct PreflightOptions {
    pub check_format: bool,
    pub check_warnings: bool,
    pub check_tests: bool,
    pub check_audit: bool,
    pub check_unwraps: bool,
    pub check_bin_size: bool,
    pub max_bin_size: u64,
}
