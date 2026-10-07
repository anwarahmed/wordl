//! The parts of wordl that another game can be built on: the rules, the word lists with
//! their definitions, the saved files and the self-updater. The game itself (`main.rs`
//! and the modules it declares) is one user of this library; funwordl
//! (https://github.com/anwarahmed/funwordl) is another, so a word or a rule fixed here
//! is fixed for both.
//!
//! Nothing in here knows what the screen looks like, and nothing assumes the program is
//! called wordl: the name, where it is released and where it keeps its files are passed
//! in (`update::Program`, `store::state_dir`).

pub mod game;
pub mod store;
pub mod update;
pub mod words;
