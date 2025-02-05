pub struct ExampleState {
  value: i32,
}

impl ExampleState {
  pub fn new() -> Self {
      Self { value: 0 }
  }

  pub fn increment(&mut self) {
      self.value += 1;
  }

  pub fn decrement(&mut self) {
      self.value -= 1;
  }

  pub fn get_value(&self) -> i32 {
      self.value
  }
}
