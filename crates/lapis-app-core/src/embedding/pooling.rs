use super::{invalid, unavailable};
use crate::services::Result;
#[cfg(test)]
mod tests;

pub(super) fn pool(
    shape: &[i64],
    data: &[f32],
    mask: &[i64],
    rows: usize,
    columns: usize,
    normalize: bool,
) -> Result<Vec<Vec<f64>>> {
    let dimensions = match shape {
        [batch, tokens, dimensions] if *batch == rows as i64 && *tokens == columns as i64 => {
            *dimensions
        }
        [batch, dimensions] if *batch == rows as i64 => *dimensions,
        _ => return Err(unavailable("Invalid embedding output shape")),
    };
    if dimensions <= 0 || dimensions > 65536 {
        return Err(unavailable("Invalid embedding dimensions"));
    }
    let dimensions = dimensions as usize;
    let cells = rows
        .checked_mul(dimensions)
        .and_then(|n| n.checked_mul(if shape.len() == 3 { columns } else { 1 }))
        .ok_or_else(|| invalid("Embedding output exceeds bounds"))?;
    if cells != data.len()
        || mask.len() != rows * columns
        || mask.iter().any(|n| !matches!(n, 0 | 1))
        || data.iter().any(|n| !n.is_finite())
    {
        return Err(unavailable("Invalid embedding output tensor"));
    }
    let mut vectors = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut vector = vec![0f32; dimensions];
        if shape.len() == 2 {
            vector.copy_from_slice(&data[row * dimensions..(row + 1) * dimensions]);
        } else {
            let count = mask[row * columns..(row + 1) * columns].iter().sum::<i64>();
            if count <= 0 {
                return Err(unavailable("Embedding attention mask is empty"));
            }
            for (dimension, value) in vector.iter_mut().enumerate() {
                let sum = (0..columns)
                    .map(|column| {
                        data[(row * columns + column) * dimensions + dimension] as f64
                            * mask[row * columns + column] as f64
                    })
                    .sum::<f64>();
                *value = (sum / count as f64) as f32;
            }
        }
        if normalize {
            let norm = vector
                .iter()
                .map(|v| (*v as f64).powi(2))
                .sum::<f64>()
                .sqrt() as f32;
            if !norm.is_finite() {
                return Err(unavailable("Embedding norm exceeds float32 bounds"));
            }
            if norm > 0.0 {
                for value in &mut vector {
                    *value = (*value as f64 / norm as f64) as f32;
                }
            }
        }
        vectors.push(vector.into_iter().map(f64::from).collect());
    }
    Ok(vectors)
}
