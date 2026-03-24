use include_dir::{include_dir, Dir};
pub static PROJECT_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR");
