use super::pool;

#[test]
fn padding_is_excluded_and_each_row_uses_its_own_mask() {
    let output = pool(
        &[2, 3, 2],
        &[1., 0., 3., 2., 999., 999., 0., 2., 999., 999., 999., 999.],
        &[1, 1, 0, 1, 0, 0],
        2,
        3,
        false,
    )
    .unwrap();
    assert_eq!(output, vec![vec![2., 1.], vec![0., 2.]]);
}
#[test]
fn pooled_vectors_follow_the_retained_float32_normalization() {
    let output = pool(&[1, 2, 2], &[3., 4., 3., 4.], &[1, 1], 1, 2, true).unwrap();
    assert!((output[0][0] - 0.6).abs() < 0.000001);
    assert!((output[0][1] - 0.8).abs() < 0.000001);
    assert_eq!(
        pool(&[1, 2], &[0., 0.], &[1], 1, 1, true).unwrap(),
        vec![vec![0., 0.]]
    );
}
#[test]
fn invalid_tensor_shapes_empty_masks_and_nonfinite_outputs_are_rejected() {
    for (shape, data, mask) in [
        (vec![1, 1, 2], vec![1.], vec![1]),
        (vec![1, 1, 2], vec![1., 2.], vec![0]),
        (vec![1, 1, 2], vec![f32::NAN, 2.], vec![1]),
        (vec![1, 1, 2], vec![f32::INFINITY, 2.], vec![1]),
        (vec![1, 1, 2], vec![1., 2.], vec![2]),
        (vec![1, 2], vec![f32::MAX, f32::MAX], vec![1]),
        (vec![1, 1, -1], vec![], vec![1]),
    ] {
        assert!(pool(&shape, &data, &mask, 1, 1, true).is_err());
    }
}
