// Fixture library with some functions to be tested
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

pub fn untested_function() -> &'static str {
    "nobody tests me"
}

pub struct MyStruct {
    value: i32,
}

impl MyStruct {
    pub fn new(value: i32) -> Self {
        MyStruct { value }
    }

    pub fn get_value(&self) -> i32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_basic() {
        let result = add(2, 3);
        assert_eq!(result, 5);
    }

    #[test]
    fn test_multiply_called_not_asserted() {
        // calls multiply but doesn't assert on it directly
        let _ = multiply(2, 3);
    }

    #[test]
    fn test_add_with_expect() {
        let v: Option<i32> = Some(add(1, 1));
        let _ = v.expect("should have value");
    }

    #[test]
    fn test_nested_braces() {
        let result = add(
            {
                let x = 1;
                x + 1
            },
            3,
        );
        assert_eq!(result, 5);
    }
}
