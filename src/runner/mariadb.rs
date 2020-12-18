#[derive(Debug)]
pub struct MariaDB;

impl crate::runner::Runner for MariaDB {
  fn run(&self) -> String {
    String::from("hello from a trait")
  }
}
