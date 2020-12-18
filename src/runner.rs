pub mod mariadb;

pub trait Runner {
  fn run(&self) -> String;
}
