pub type Mat4 = [[f32; 4]; 4];

pub trait Matrix {
    fn projection(left: f32, right: f32, top: f32, bottom: f32) -> Self;
}

impl Matrix for Mat4 {
    fn projection(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        [
            [2.0 / (right - left), 0.0, 0.0, 0.0],
            [0.0, 2.0 / (top - bottom), 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [
                -(right + left) / (right - left),
                -(top + bottom) / (top - bottom),
                0.0,
                1.0,
            ],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projection() {
        let mat = Mat4::projection(-1.0, 1.0, 1.0, -1.0);
        let expected = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert_eq!(mat, expected);
    }
}
