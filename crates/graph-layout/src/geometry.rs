/// 2D vector with f32 coordinates
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// Create a new vector
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Create a zero vector
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Return the component-wise maximum of two vectors
    pub fn max(self, other: Self) -> Self {
        Self {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }

    /// Return the sum of x and y components
    pub fn sum(self) -> f32 {
        self.x + self.y
    }
}

/// 2D point with f32 coordinates
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    /// Create a new point
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
