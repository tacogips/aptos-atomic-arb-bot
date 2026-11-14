//pub use ahash::RandomState;
//pub use hashbrown::hash_map::Values as HashMapValues;
//pub use hashbrown::HashMap;
//pub use hashbrown::HashSet;

pub use fxhash::FxBuildHasher as RandomState;
pub use fxhash::FxHashMap as HashMap;
pub use fxhash::FxHashSet as HashSet;
//pub use fxhash::Values as HashMapValues;
pub use std::collections::hash_map::Values as HashMapValues;

pub fn new_random_state() -> RandomState {
    RandomState::default()
}

//use std::collections::hash_map::Values as HashMapValues;
//
//pub use std::collections::hash_map::RandomState;
//pub use std::collections::HashMap;
//pub use std::collections::HashSet;
